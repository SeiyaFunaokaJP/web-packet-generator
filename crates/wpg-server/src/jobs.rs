//! Job manager (TODO M3 "ジョブマネージャ (複数ストリーム混合、スケジュール、キャンセル
//! 安全性)", "送出制限ポリシー").
//!
//! A job is a [`JobSpec`]: one or more template streams (by id from the
//! [`TemplateStore`] or inline) mixed weighted / sequentially / round-robin, sent
//! on one interface through one backend. Creating a job compiles and renders its
//! streams and checks the transmit policy (`tx.allowed_ifaces`, `tx.max_rate`);
//! starting it opens the backend via [`TxJob::start_on`]. A tokio task per running
//! job samples [`JobHandle::stats`] every 100 ms and publishes
//! [`ServerEvent::Stats`] / [`ServerEvent::JobState`]; when the job ends its
//! [`JobReport`] is stored, kept in a ring of the last [`HISTORY_MAX`] finished
//! jobs and persisted in the `job_history` table of the database ([`Db`]).
//!
//! Cancel safety: the manager owns every [`JobHandle`]; deleting a job, calling
//! [`JobManager::shutdown`] or dropping the last clone of the manager stops the
//! TX threads and joins them (the monitor tasks only hold weak references).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wpg_core::stream::{Expansion, Plan, PlanConfig, PlanHint};
use wpg_protocols::Catalog;
use wpg_template::{apply_overrides, compile, Compiled, Template};
use wpg_tx::backend;
use wpg_tx::job::{
    FinishReason, JobConfig, JobHandle, JobReport, JobState, SignatureConfig, StatsSnapshot, TxJob,
};
use wpg_tx::rate::{link_speed_bps, parse_duration, parse_start_time, Rate};
use wpg_tx::source::{FrameSource, MixMode, Mixer, PreRendered, Rendered};
use wpg_tx::TxOptions;

use crate::config::Config;
use crate::db::Db;
use crate::error::{codes, ApiError};
use crate::events::{EventBus, ServerEvent};
use crate::store::{StoreError, TemplateStore};

/// Finished jobs kept in memory.
pub const HISTORY_MAX: usize = 100;

/// Default statistics cadence.
pub const SNAPSHOT_INTERVAL: Duration = Duration::from_millis(100);

const fn one() -> u32 {
    1
}

/// One stream of a job: a stored template (`template_id`) or an inline one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct JobStream {
    /// Id of a stored template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    /// Inline template (takes precedence over `template_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub template: Option<Template>,
    /// Template variable values (`name` → human notation).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, String>,
    /// Weight for `mix: weighted` (0 counts as 1).
    #[serde(default = "one")]
    pub weight: u32,
}

/// How several streams are interleaved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mix {
    /// Smooth weighted round-robin by `weight`.
    #[default]
    Weighted,
    /// One stream after the other.
    Sequential,
    /// One frame of each stream in turn.
    RoundRobin,
}

impl From<Mix> for MixMode {
    fn from(m: Mix) -> Self {
        match m {
            Mix::Weighted => MixMode::Weighted,
            Mix::Sequential => MixMode::Sequential,
            Mix::RoundRobin => MixMode::RoundRobin,
        }
    }
}

/// What to send, where and how (all fields but `streams` are optional).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct JobSpec {
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Streams (at least one). Several streams are mixed into one TX worker.
    pub streams: Vec<JobStream>,
    /// Interleaving of several streams.
    pub mix: Mix,
    /// Interface (not needed for `null`; the output file name for `pcapfile`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iface: Option<String>,
    /// Backend: `null`, `pcapfile`, `afpacket`, `pcap`, ... (default: `tx.default_backend`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Rate: `1000pps`, `1Mpps`, `10Gbps`, `50%`, `ifg=10us`, `unlimited`
    /// (default: the first template's `tx.rate`, then `tx.max_rate`, else unlimited).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<String>,
    /// Total packets (default: the template's `tx.count` for a single stream).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Duration limit, e.g. `10s`, `2m`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Frames per rate-control grant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst: Option<u32>,
    /// TX worker threads (single stream only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<usize>,
    /// Core per worker (Linux).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pin_cores: Vec<usize>,
    /// Linear rate ramp-up time, e.g. `5s`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ramp_up: Option<String>,
    /// Scheduled start: RFC 3339 or `+10s`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    /// Dry run: write a pcapng file of this name under `<data_dir>/captures`
    /// instead of sending (forces the `pcapfile` backend).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<String>,
    /// Expansion strategy: `auto`, `prerender`, `onthefly`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Frames per `send_batch` call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<usize>,
    /// Stamp every frame with a receive-verification signature (magic + stream
    /// id + sequence number + TX time over the last 24 payload octets; see
    /// `/api/captures` with `verify`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub signature: bool,
    /// Signature stream id of the first stream (stream `i` gets `stream_id + i`;
    /// default 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<u32>,
}

/// Job lifecycle state: `{ "state": "running" }`, `{ "state": "finished",
/// "detail": "count_reached" }`, `{ "state": "failed", "detail": "<error>" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum JobStatus {
    /// Created with `start: false`, not started yet.
    Created,
    /// Waiting for `start_at`.
    Scheduled,
    /// Sending.
    Running,
    /// Paused.
    Paused,
    /// Ended normally (`count_reached`, `duration_elapsed`, `source_exhausted`,
    /// `stopped`).
    Finished(EndReason),
    /// Ended with an error.
    Failed(String),
}

impl JobStatus {
    /// Whether the job has ended.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Finished(_) | JobStatus::Failed(_))
    }

    /// Short name (`running`, ...).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            JobStatus::Created => "created",
            JobStatus::Scheduled => "scheduled",
            JobStatus::Running => "running",
            JobStatus::Paused => "paused",
            JobStatus::Finished(_) => "finished",
            JobStatus::Failed(_) => "failed",
        }
    }
}

/// Why a job ended normally (mirrors `wpg_tx::job::FinishReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// The packet count was reached.
    CountReached,
    /// The duration elapsed.
    DurationElapsed,
    /// The streams have no more packets.
    SourceExhausted,
    /// Stopped by the user (or at shutdown).
    Stopped,
}

impl From<FinishReason> for EndReason {
    fn from(r: FinishReason) -> Self {
        match r {
            FinishReason::CountReached => EndReason::CountReached,
            FinishReason::DurationElapsed => EndReason::DurationElapsed,
            FinishReason::SourceExhausted => EndReason::SourceExhausted,
            FinishReason::Stopped => EndReason::Stopped,
        }
    }
}

impl From<&JobState> for JobStatus {
    fn from(s: &JobState) -> Self {
        match s {
            JobState::Scheduled => JobStatus::Scheduled,
            JobState::Running => JobStatus::Running,
            JobState::Paused => JobStatus::Paused,
            JobState::Finished(r) => JobStatus::Finished((*r).into()),
            JobState::Failed(m) => JobStatus::Failed(m.clone()),
        }
    }
}

/// A job as returned by the API (also the persisted history record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct JobInfo {
    /// Job id.
    pub id: String,
    /// Display name (`spec.name`, else the first template's name).
    pub name: String,
    /// The request.
    pub spec: JobSpec,
    /// Current state.
    pub status: JobStatus,
    /// Resolved backend.
    pub backend: String,
    /// Resolved interface (output path for `pcapfile`).
    pub iface: String,
    /// Resolved rate.
    pub rate: String,
    /// TX workers.
    pub workers: usize,
    /// Expansion plan of each stream.
    #[schema(value_type = Vec<Object>)]
    pub plans: Vec<Plan>,
    /// Creation time, Unix milliseconds.
    pub created: u64,
    /// Start time, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started: Option<u64>,
    /// End time, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished: Option<u64>,
    /// Latest statistics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub stats: Option<StatsSnapshot>,
    /// Final report (after the job ended).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub report: Option<JobReport>,
}

struct Prepared {
    sources: Vec<Box<dyn FrameSource>>,
    config: JobConfig,
    opts: TxOptions,
}

struct JobEntry {
    info: JobInfo,
    prepared: Option<Prepared>,
    handle: Option<JobHandle>,
}

struct Inner {
    jobs: Mutex<BTreeMap<String, JobEntry>>,
    history: Mutex<VecDeque<JobInfo>>,
    db: Db,
    events: EventBus,
    config: Arc<Config>,
    store: TemplateStore,
    catalog: &'static Catalog,
    interval: Duration,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Stop every job first so that the joins in `JobHandle::drop` are quick.
        let jobs = self.jobs.get_mut().unwrap_or_else(PoisonError::into_inner);
        for e in jobs.values() {
            if let Some(h) = &e.handle {
                h.stop();
            }
        }
    }
}

/// The job manager (cheap to clone).
#[derive(Clone)]
pub struct JobManager {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for JobManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobManager")
            .field("jobs", &lock(&self.inner.jobs).len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn spec_error(e: impl ToString) -> ApiError {
    ApiError::bad_request("api-error-job-spec", e)
}

fn backend_error(e: impl ToString) -> ApiError {
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        codes::BACKEND_ERROR,
        "api-error-backend",
    )
    .arg("error", e)
}

/// Maps a store error to an API error.
#[must_use]
pub fn store_error(e: StoreError) -> ApiError {
    match e {
        StoreError::NotFound(id) => ApiError::not_found(format!("template {id}")),
        StoreError::ReadOnly(id) => ApiError::new(
            StatusCode::FORBIDDEN,
            codes::READ_ONLY,
            "api-error-read-only",
        )
        .arg("id", id),
        StoreError::Broken { id, error } => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            codes::INVALID_TEMPLATE,
            "api-error-invalid-template",
        )
        .arg("error", format!("{id}: {error}")),
        StoreError::Template(t) => ApiError::template(t),
        e @ StoreError::Io { .. } => ApiError::internal(e),
    }
}

/// Resolves a dry-run / capture file name under `dir`; rejects absolute paths and `..`.
pub(crate) fn capture_path(dir: &Path, name: &str) -> Result<PathBuf, ApiError> {
    let p = Path::new(name);
    let ok = !name.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)));
    if !ok {
        return Err(spec_error(format!(
            "file names must be relative, without `..` (got `{name}`)"
        )));
    }
    let mut path = dir.join(p);
    if path.extension().is_none() {
        path.set_extension("pcapng");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ApiError::internal)?;
    }
    Ok(path)
}

fn plan_hint(s: Option<&str>) -> Result<PlanHint, ApiError> {
    match s
        .unwrap_or("auto")
        .to_ascii_lowercase()
        .replace(['-', '_'], "")
        .as_str()
    {
        "auto" => Ok(PlanHint::Auto),
        "prerender" | "pre" => Ok(PlanHint::PreRender),
        "onthefly" | "fly" => Ok(PlanHint::OnTheFly),
        other => Err(spec_error(format!(
            "unknown plan `{other}` (auto, prerender, onthefly)"
        ))),
    }
}

/// Length of the first frame of a compiled stream.
fn first_frame_len(c: &Compiled, catalog: &Catalog) -> Result<usize, ApiError> {
    let exp = Expansion::new(&c.stream, None).map_err(|e| ApiError::template(e.into()))?;
    let mut p = c.packet.clone();
    if !exp.is_empty() {
        let o = exp.nth(0).map_err(|e| ApiError::template(e.into()))?;
        apply_overrides(&mut p, &o).map_err(ApiError::template)?;
    }
    Ok(p.build(catalog)
        .map_err(|e| ApiError::template(e.into()))?
        .bytes
        .len())
}

impl Inner {
    fn events_send(&self, ev: ServerEvent) {
        let _ = self.events.send(ev);
    }

    /// Validates `spec`, applies the policy and renders the sources.
    #[allow(clippy::too_many_lines)]
    fn prepare(&self, spec: &JobSpec) -> Result<(JobInfo, Prepared), ApiError> {
        let cfg = &self.config;
        if spec.streams.is_empty() {
            return Err(spec_error("a job needs at least one stream"));
        }
        let backend = match (&spec.dry_run, &spec.backend) {
            (Some(_), _) => "pcapfile".to_owned(),
            (None, Some(b)) => b.clone(),
            (None, None) => cfg.default_backend(),
        };
        let exempt = matches!(backend.as_str(), "null" | "pcapfile");
        let iface = match backend.as_str() {
            "pcapfile" => {
                let name = spec
                    .dry_run
                    .as_deref()
                    .or(spec.iface.as_deref())
                    .ok_or_else(|| {
                        spec_error("the pcapfile backend needs `dry_run` (file name)")
                    })?;
                capture_path(&cfg.capture_dir(), name)?
                    .display()
                    .to_string()
            }
            "null" => spec.iface.clone().unwrap_or_else(|| "null0".to_owned()),
            _ => spec
                .iface
                .clone()
                .ok_or_else(|| spec_error(format!("backend `{backend}` needs `iface`")))?,
        };
        if !exempt && !cfg.tx.allowed_ifaces.is_empty() && !cfg.tx.allowed_ifaces.contains(&iface) {
            return Err(ApiError::policy("api-error-policy-iface")
                .arg("iface", &iface)
                .arg("allowed", cfg.tx.allowed_ifaces.join(", ")));
        }
        let workers = spec.workers.unwrap_or(1);
        if workers == 0 {
            return Err(spec_error("workers must be at least 1"));
        }
        if workers > 1 && spec.streams.len() > 1 {
            return Err(spec_error(
                "several streams are mixed into one TX worker (workers must be 1)",
            ));
        }
        if workers > 1 && !backend::supports_workers(&backend) {
            return Err(spec_error(format!(
                "backend `{backend}` supports a single worker"
            )));
        }

        // Compile the templates.
        let mut compiled = Vec::with_capacity(spec.streams.len());
        let mut names = Vec::new();
        for (i, s) in spec.streams.iter().enumerate() {
            let t = match (&s.template, &s.template_id) {
                (Some(t), _) => t.clone(),
                (None, Some(id)) => {
                    let (_, t) = self.store.get(id).map_err(store_error)?;
                    self.store.mark_used(id);
                    t
                }
                (None, None) => {
                    return Err(spec_error(format!(
                        "streams[{i}] needs `template_id` or `template`"
                    )))
                }
            };
            names.push(t.meta.name.clone());
            compiled.push(compile(&t, &s.vars, self.catalog).map_err(ApiError::template)?);
        }
        let single = compiled.len() == 1;
        let tx0 = compiled[0].tx.clone();

        // Rate, limits.
        let rate_text = spec.rate.clone().or_else(|| tx0.rate.clone());
        let mut rate = match &rate_text {
            Some(r) => Rate::parse(r).map_err(|e| spec_error(format!("rate: {e}")))?,
            None => Rate::Unlimited,
        };
        if let Rate::LinkPercent { percent, .. } = rate {
            let link = link_speed_bps(&iface).ok_or_else(|| {
                spec_error(format!(
                    "`{percent}%` needs the link speed of `{iface}`, which is unknown; give an absolute rate"
                ))
            })?;
            rate = rate.with_link_speed(link);
        }
        let min_len = compiled
            .iter()
            .map(|c| first_frame_len(c, self.catalog))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .unwrap_or(60);
        if let (false, Some(max_text)) = (exempt, &cfg.tx.max_rate) {
            let max = Rate::parse(max_text).map_err(ApiError::internal)?;
            if !max.is_unlimited() {
                if rate.is_unlimited() && rate_text.is_none() {
                    rate = max;
                } else {
                    let want = rate.frames_per_second(min_len).unwrap_or(f64::INFINITY);
                    let limit = max.frames_per_second(min_len).unwrap_or(f64::INFINITY);
                    if want > limit * (1.0 + 1e-9) {
                        let key = if rate.is_unlimited() {
                            "api-error-policy-unlimited"
                        } else {
                            "api-error-policy-rate"
                        };
                        return Err(ApiError::policy(key).arg("rate", rate).arg("max", max_text));
                    }
                }
            }
        }
        let count = spec.count.or(if single { tx0.count } else { None });
        let duration = match &spec.duration {
            Some(d) => Some(parse_duration(d).map_err(|e| spec_error(format!("duration: {e}")))?),
            None if single => tx0
                .duration_secs
                .map(Duration::try_from_secs_f64)
                .transpose()
                .map_err(|e| spec_error(format!("duration: {e}")))?,
            None => None,
        };
        let ramp_up = spec
            .ramp_up
            .as_deref()
            .map(|s| parse_duration(s).map_err(|e| spec_error(format!("ramp_up: {e}"))))
            .transpose()?;
        let start_at = spec
            .start_at
            .as_deref()
            .map(|s| parse_start_time(s, SystemTime::now()).map_err(spec_error))
            .transpose()?;
        let burst = spec.burst.or(tx0.burst).unwrap_or(1).max(1);
        let hint = plan_hint(spec.plan.as_deref())?;

        // Sources. A single stream follows `wpg send`: an explicit count makes the
        // generators cycle up to that many packets, a duration alone loops the
        // stream. Mixed streams loop whenever the job has a count or duration.
        let loop_each = if single {
            count.is_none() && duration.is_some()
        } else {
            count.is_some() || duration.is_some()
        };
        let pcfg = PlanConfig {
            hint,
            ..PlanConfig::default()
        };
        let mut rendered = Vec::with_capacity(compiled.len());
        let mut plans = Vec::with_capacity(compiled.len());
        for mut c in compiled {
            if let (true, Some(n)) = (single, count) {
                c.stream.count = Some(n);
            }
            let exp = Expansion::new(&c.stream, None).map_err(|e| ApiError::template(e.into()))?;
            let (r, plan) = Rendered::auto(c.packet, exp, self.catalog, &pcfg)
                .map_err(|e| ApiError::bad_request("api-error-render-failed", e))?;
            let r = match (r, loop_each) {
                (Rendered::Pre(p), true) => Rendered::Pre(PreRendered::new(p.into_frames(), None)),
                (Rendered::Fly(f), true) => Rendered::Fly(Box::new(f.cycling())),
                (other, false) => other,
            };
            rendered.push(r);
            plans.push(plan);
        }
        let sources: Vec<Box<dyn FrameSource>> = if single {
            rendered
                .pop()
                .map(|r| r.split(workers))
                .unwrap_or_default()
                .into_iter()
                .map(|s| Box::new(s) as Box<dyn FrameSource>)
                .collect()
        } else {
            let mut m = Mixer::new(spec.mix.into());
            for (r, s) in rendered.into_iter().zip(&spec.streams) {
                m.push(Box::new(r), s.weight);
            }
            vec![Box::new(m)]
        };
        let batch = spec.batch.unwrap_or(64).max(1);
        let opts = TxOptions {
            batch_size: batch,
            eal_args: cfg.tx.dpdk_eal_args.clone(),
            ..TxOptions::default()
        };
        let config = JobConfig {
            count,
            duration,
            rate,
            burst,
            pin_cores: spec.pin_cores.clone(),
            workers: sources.len(),
            batch_size: batch,
            ramp_up,
            start_at,
            snapshot_interval: self.interval,
            signature: spec.signature.then(|| SignatureConfig {
                stream_id_base: spec.stream_id.unwrap_or(0),
            }),
            ..JobConfig::default()
        };
        let info = JobInfo {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: spec.name.clone().unwrap_or_else(|| names.join(" + ")),
            spec: spec.clone(),
            status: JobStatus::Created,
            backend,
            iface,
            rate: rate.to_string(),
            workers: sources.len(),
            plans,
            created: now_ms(),
            started: None,
            finished: None,
            stats: None,
            report: None,
        };
        Ok((
            info,
            Prepared {
                sources,
                config,
                opts,
            },
        ))
    }

    fn record_history(&self, info: &JobInfo) {
        {
            let mut h = lock(&self.history);
            h.retain(|j| j.id != info.id);
            h.push_front(info.clone());
            h.truncate(HISTORY_MAX);
        }
        if let Err(e) = self.db.save_job(info) {
            tracing::warn!("cannot persist job {} in the history: {e}", info.id);
        }
    }

    /// Stores the final report of a job that ended.
    fn finish(&self, id: &str, report: Result<JobReport, String>) {
        let info = {
            let mut jobs = lock(&self.jobs);
            let Some(e) = jobs.get_mut(id) else { return };
            match report {
                Ok(r) => {
                    e.info.status = JobStatus::from(&r.state);
                    e.info.stats = Some(r.stats.clone());
                    e.info.report = Some(r);
                }
                Err(m) => e.info.status = JobStatus::Failed(m),
            }
            e.info.finished = Some(now_ms());
            e.info.clone()
        };
        self.events_send(ServerEvent::JobState {
            job_id: id.to_owned(),
            state: info.status.clone(),
        });
        tracing::info!(job = %id, state = info.status.name(), "job ended");
        self.record_history(&info);
        self.evict_finished();
    }

    /// Keeps at most [`HISTORY_MAX`] finished jobs in the live list.
    fn evict_finished(&self) {
        let mut jobs = lock(&self.jobs);
        let mut done: Vec<(u64, String)> = jobs
            .values()
            .filter(|e| e.info.status.is_terminal() && e.handle.is_none())
            .map(|e| (e.info.finished.unwrap_or(0), e.info.id.clone()))
            .collect();
        if done.len() > HISTORY_MAX {
            done.sort();
            for (_, id) in done.iter().take(done.len() - HISTORY_MAX) {
                jobs.remove(id);
            }
        }
    }
}

/// Samples a running job until it ends.
async fn monitor(weak: Weak<Inner>, id: String, interval: Duration) {
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let Some(inner) = weak.upgrade() else { return };
        let handle = {
            let mut jobs = lock(&inner.jobs);
            let Some(e) = jobs.get_mut(&id) else { return };
            let Some(h) = e.handle.as_ref() else { return };
            // The bounded snapshot queue is only drained; `stats()` has the latest.
            while h.snapshots().try_recv().is_ok() {}
            let finished = h.is_finished();
            let snap = h.stats();
            let status = JobStatus::from(&snap.state);
            e.info.stats = Some(snap.clone());
            inner.events_send(ServerEvent::Stats {
                job_id: id.clone(),
                snapshot: snap,
            });
            if status != e.info.status && !status.is_terminal() {
                e.info.status = status.clone();
                inner.events_send(ServerEvent::JobState {
                    job_id: id.clone(),
                    state: status,
                });
            }
            if finished {
                e.handle.take()
            } else {
                None
            }
        };
        if let Some(h) = handle {
            let report = tokio::task::spawn_blocking(move || h.wait())
                .await
                .map_err(|e| format!("TX thread panicked: {e}"));
            inner.finish(&id, report);
            return;
        }
    }
}

/// Dashboard aggregate of the running jobs.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct JobTotals {
    /// Jobs that are scheduled, running or paused.
    pub active: usize,
    /// Jobs currently sending.
    pub running: usize,
    /// Sum of the jobs' packets per second.
    pub pps: f64,
    /// Sum of the jobs' L2 bits per second.
    pub bps: f64,
    /// Sum of the jobs' L1 bits per second.
    pub l1_bps: f64,
}

impl JobManager {
    /// Creates the manager and loads the persisted history from `db` (importing
    /// the JSON records of older versions from `<data_dir>/history` once).
    #[must_use]
    pub fn new(
        config: Arc<Config>,
        store: TemplateStore,
        catalog: &'static Catalog,
        events: EventBus,
        db: Db,
    ) -> Self {
        Self::with_interval(config, store, catalog, events, db, SNAPSHOT_INTERVAL)
    }

    /// [`Self::new`] with a custom statistics cadence.
    #[must_use]
    pub fn with_interval(
        config: Arc<Config>,
        store: TemplateStore,
        catalog: &'static Catalog,
        events: EventBus,
        db: Db,
        interval: Duration,
    ) -> Self {
        match db.import_legacy_history(&config.history_dir()) {
            Ok(0) => {}
            Ok(n) => tracing::info!("imported {n} job records into the database"),
            Err(e) => tracing::warn!("cannot import the old job history: {e}"),
        }
        let history: VecDeque<JobInfo> = db
            .jobs(HISTORY_MAX)
            .unwrap_or_else(|e| {
                tracing::warn!("cannot load the job history: {e}");
                Vec::new()
            })
            .into();
        Self {
            inner: Arc::new(Inner {
                jobs: Mutex::new(BTreeMap::new()),
                history: Mutex::new(history),
                db,
                events,
                config,
                store,
                catalog,
                interval,
            }),
        }
    }

    /// Validates and prepares a job; starts it unless `start` is false.
    ///
    /// # Errors
    /// `bad_request` / `invalid_template` for a bad spec, `policy_violation` when
    /// the policy forbids it, `backend_error` when the backend cannot be opened
    /// (the job is then discarded).
    pub async fn create(&self, spec: JobSpec, start: bool) -> Result<JobInfo, ApiError> {
        let inner = Arc::clone(&self.inner);
        let (info, prepared) = tokio::task::spawn_blocking(move || inner.prepare(&spec))
            .await
            .map_err(ApiError::internal)??;
        let id = info.id.clone();
        lock(&self.inner.jobs).insert(
            id.clone(),
            JobEntry {
                info,
                prepared: Some(prepared),
                handle: None,
            },
        );
        tracing::info!(job = %id, "job created");
        if start {
            if let Err(e) = self.start(&id).await {
                lock(&self.inner.jobs).remove(&id);
                return Err(e);
            }
        }
        self.get(&id)
    }

    /// Starts a created job.
    ///
    /// # Errors
    /// `not_found`, `invalid_state` (not in state `created`), `backend_error`.
    pub async fn start(&self, id: &str) -> Result<JobInfo, ApiError> {
        let (prepared, backend, iface) = {
            let mut jobs = lock(&self.inner.jobs);
            let e = jobs
                .get_mut(id)
                .ok_or_else(|| ApiError::not_found(format!("job {id}")))?;
            match e.prepared.take() {
                Some(p) if e.info.status == JobStatus::Created => {
                    (p, e.info.backend.clone(), e.info.iface.clone())
                }
                other => {
                    e.prepared = other;
                    return Err(ApiError::invalid_state(id, e.info.status.name(), "start"));
                }
            }
        };
        let started = tokio::task::spawn_blocking(move || {
            TxJob::start_on(
                &backend,
                &iface,
                &prepared.opts,
                prepared.sources,
                prepared.config,
            )
        })
        .await
        .map_err(ApiError::internal)?;
        let mut jobs = lock(&self.inner.jobs);
        let e = jobs
            .get_mut(id)
            .ok_or_else(|| ApiError::not_found(format!("job {id}")))?;
        match started {
            Ok(h) => {
                // Terminal states are only recorded by the monitor (with the report).
                e.info.status = match JobStatus::from(&h.state()) {
                    s if s.is_terminal() => JobStatus::Running,
                    s => s,
                };
                e.info.started = Some(now_ms());
                e.handle = Some(h);
                let info = e.info.clone();
                drop(jobs);
                self.inner.events_send(ServerEvent::JobState {
                    job_id: id.to_owned(),
                    state: info.status.clone(),
                });
                tracing::info!(job = %id, backend = %info.backend, iface = %info.iface, "job started");
                tokio::spawn(monitor(
                    Arc::downgrade(&self.inner),
                    id.to_owned(),
                    self.inner.interval,
                ));
                Ok(info)
            }
            Err(err) => {
                e.info.status = JobStatus::Failed(err.to_string());
                e.info.finished = Some(now_ms());
                let info = e.info.clone();
                drop(jobs);
                self.inner.events_send(ServerEvent::JobState {
                    job_id: id.to_owned(),
                    state: info.status.clone(),
                });
                self.inner.record_history(&info);
                Err(backend_error(err))
            }
        }
    }

    fn with_handle(
        &self,
        id: &str,
        action: &str,
        f: impl FnOnce(&JobHandle),
    ) -> Result<JobInfo, ApiError> {
        let mut jobs = lock(&self.inner.jobs);
        let e = jobs
            .get_mut(id)
            .ok_or_else(|| ApiError::not_found(format!("job {id}")))?;
        match &e.handle {
            Some(h) if !e.info.status.is_terminal() => {
                f(h);
                let st = JobStatus::from(&h.state());
                if !st.is_terminal() {
                    e.info.status = st;
                }
                Ok(e.info.clone())
            }
            _ => Err(ApiError::invalid_state(id, e.info.status.name(), action)),
        }
    }

    /// Pauses a running job.
    ///
    /// # Errors
    /// `not_found`, `invalid_state`.
    pub fn pause(&self, id: &str) -> Result<JobInfo, ApiError> {
        self.with_handle(id, "pause", JobHandle::pause)
    }

    /// Resumes a paused job.
    ///
    /// # Errors
    /// `not_found`, `invalid_state`.
    pub fn resume(&self, id: &str) -> Result<JobInfo, ApiError> {
        self.with_handle(id, "resume", JobHandle::resume)
    }

    /// Requests a job to stop (returns immediately; the state becomes
    /// `finished/stopped` shortly after). A created job that never started is
    /// finished right away.
    ///
    /// # Errors
    /// `not_found`, `invalid_state` for a job that already ended.
    pub fn stop(&self, id: &str) -> Result<JobInfo, ApiError> {
        {
            let mut jobs = lock(&self.inner.jobs);
            let e = jobs
                .get_mut(id)
                .ok_or_else(|| ApiError::not_found(format!("job {id}")))?;
            if e.info.status == JobStatus::Created {
                e.prepared = None;
                e.info.status = JobStatus::Finished(EndReason::Stopped);
                e.info.finished = Some(now_ms());
                let info = e.info.clone();
                drop(jobs);
                self.inner.events_send(ServerEvent::JobState {
                    job_id: id.to_owned(),
                    state: info.status.clone(),
                });
                self.inner.record_history(&info);
                return Ok(info);
            }
        }
        self.with_handle(id, "stop", JobHandle::stop)
    }

    /// Removes a job; a running one is stopped and its threads joined first.
    ///
    /// # Errors
    /// `not_found`.
    pub async fn delete(&self, id: &str) -> Result<(), ApiError> {
        let entry = lock(&self.inner.jobs)
            .remove(id)
            .ok_or_else(|| ApiError::not_found(format!("job {id}")))?;
        if let Some(h) = entry.handle {
            let mut info = entry.info;
            let report = tokio::task::spawn_blocking(move || h.stop_and_wait())
                .await
                .map_err(ApiError::internal)?;
            info.status = JobStatus::from(&report.state);
            info.stats = Some(report.stats.clone());
            info.report = Some(report);
            info.finished = Some(now_ms());
            self.inner.events_send(ServerEvent::JobState {
                job_id: id.to_owned(),
                state: info.status.clone(),
            });
            self.inner.record_history(&info);
        }
        tracing::info!(job = %id, "job deleted");
        Ok(())
    }

    /// All live jobs (created, running and recently finished), newest first.
    #[must_use]
    pub fn list(&self) -> Vec<JobInfo> {
        let mut v: Vec<JobInfo> = lock(&self.inner.jobs)
            .values()
            .map(|e| e.info.clone())
            .collect();
        v.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
        v
    }

    /// One job (live, else from the history).
    ///
    /// # Errors
    /// `not_found`.
    pub fn get(&self, id: &str) -> Result<JobInfo, ApiError> {
        if let Some(e) = lock(&self.inner.jobs).get(id) {
            let mut info = e.info.clone();
            if let Some(h) = &e.handle {
                info.stats = Some(h.stats());
            }
            return Ok(info);
        }
        lock(&self.inner.history)
            .iter()
            .find(|j| j.id == id)
            .cloned()
            .ok_or_else(|| ApiError::not_found(format!("job {id}")))
    }

    /// Latest statistics of a job.
    ///
    /// # Errors
    /// `not_found`, or `invalid_state` for a job that never started.
    pub fn stats(&self, id: &str) -> Result<StatsSnapshot, ApiError> {
        let info = self.get(id)?;
        info.stats
            .ok_or_else(|| ApiError::invalid_state(id, info.status.name(), "stats"))
    }

    /// Finished jobs, newest first.
    #[must_use]
    pub fn history(&self) -> Vec<JobInfo> {
        lock(&self.inner.history).iter().cloned().collect()
    }

    /// Sum of the rates of the running jobs.
    #[must_use]
    pub fn totals(&self) -> JobTotals {
        let jobs = lock(&self.inner.jobs);
        let mut t = JobTotals::default();
        for e in jobs.values() {
            if e.handle.is_some() && !e.info.status.is_terminal() {
                t.active += 1;
                if e.info.status == JobStatus::Running {
                    t.running += 1;
                }
                if let Some(s) = &e.info.stats {
                    t.pps += s.pps;
                    t.bps += s.bps;
                    t.l1_bps += s.l1_bps;
                }
            }
        }
        t
    }

    /// Stops every job, waits for the TX threads and records the reports
    /// (graceful shutdown).
    pub async fn shutdown(&self) {
        let handles: Vec<(String, JobHandle)> = {
            let mut jobs = lock(&self.inner.jobs);
            jobs.iter_mut()
                .filter_map(|(id, e)| e.handle.take().map(|h| (id.clone(), h)))
                .collect()
        };
        for (_, h) in &handles {
            h.stop();
        }
        for (id, h) in handles {
            let report = tokio::task::spawn_blocking(move || h.wait())
                .await
                .map_err(|e| format!("TX thread panicked: {e}"));
            self.inner.finish(&id, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_paths_stay_inside() {
        let d = tempfile::tempdir().unwrap();
        let p = capture_path(d.path(), "run1").unwrap();
        assert_eq!(p, d.path().join("run1.pcapng"));
        assert!(capture_path(d.path(), "../x").is_err());
        assert!(capture_path(d.path(), "/etc/passwd").is_err());
        assert!(capture_path(d.path(), "").is_err());
    }

    #[test]
    fn status_serialisation() {
        let s = serde_json::to_value(JobStatus::Finished(EndReason::CountReached)).unwrap();
        assert_eq!(
            s,
            serde_json::json!({"state": "finished", "detail": "count_reached"})
        );
        let s = serde_json::to_value(JobStatus::Running).unwrap();
        assert_eq!(s, serde_json::json!({"state": "running"}));
    }
}
