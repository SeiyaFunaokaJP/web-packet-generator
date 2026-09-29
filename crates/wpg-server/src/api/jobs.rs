//! `/api/jobs` and `/api/stats/overview`.

// utoipa's generated code spells out paths the lint considers redundant.
#![allow(unused_qualifications)]

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::error::{ApiError, ApiJson, ApiPath, ApiResult};
use crate::jobs::{JobInfo, JobSpec, JobStatus, JobTotals};
use crate::state::AppState;

const fn yes() -> bool {
    true
}

/// `POST /api/jobs` body: a [`JobSpec`] plus `start`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateJob {
    /// The job.
    #[serde(flatten)]
    pub spec: JobSpec,
    /// Start immediately (default `true`); `false` leaves it in state `created`.
    #[serde(default = "yes")]
    pub start: bool,
}

/// All live jobs (created, running and recently finished), newest first.
#[utoipa::path(get, path = "/api/jobs", tag = "jobs",
    responses((status = 200, description = "Jobs", body = Vec<JobInfo>)))]
pub async fn list(State(st): State<AppState>) -> Json<Vec<JobInfo>> {
    Json(st.jobs.list())
}

/// Creates (and by default starts) a job.
#[utoipa::path(post, path = "/api/jobs", tag = "jobs", request_body = CreateJob,
    responses(
        (status = 201, description = "Created", body = JobInfo),
        (status = 400, description = "Invalid job", body = crate::error::ErrorBody),
        (status = 403, description = "`policy_violation`: interface or rate not allowed", body = crate::error::ErrorBody),
        (status = 404, description = "Unknown template id", body = crate::error::ErrorBody),
        (status = 422, description = "Invalid template", body = crate::error::ErrorBody),
        (status = 502, description = "`backend_error`: the backend could not be opened", body = crate::error::ErrorBody)))]
pub async fn create(
    State(st): State<AppState>,
    ApiJson(req): ApiJson<CreateJob>,
) -> ApiResult<(StatusCode, Json<JobInfo>)> {
    let info = st.jobs.create(req.spec, req.start).await?;
    Ok((StatusCode::CREATED, Json(info)))
}

/// One job (live or from the history).
#[utoipa::path(get, path = "/api/jobs/{id}", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "Job", body = JobInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn get(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<JobInfo>> {
    st.jobs.get(&id).map(Json)
}

/// Starts a job created with `start: false`.
#[utoipa::path(post, path = "/api/jobs/{id}/start", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "Started", body = JobInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Not in state `created`", body = crate::error::ErrorBody),
        (status = 502, description = "Backend error", body = crate::error::ErrorBody)))]
pub async fn start(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<JobInfo>> {
    st.jobs.start(&id).await.map(Json)
}

/// Pauses a running job.
#[utoipa::path(post, path = "/api/jobs/{id}/pause", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "Pausing", body = JobInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Not running", body = crate::error::ErrorBody)))]
pub async fn pause(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<JobInfo>> {
    st.jobs.pause(&id).map(Json)
}

/// Resumes a paused job.
#[utoipa::path(post, path = "/api/jobs/{id}/resume", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "Resuming", body = JobInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Not running", body = crate::error::ErrorBody)))]
pub async fn resume(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<JobInfo>> {
    st.jobs.resume(&id).map(Json)
}

/// Stops a job (asynchronously; watch `job_state` or poll the job).
#[utoipa::path(post, path = "/api/jobs/{id}/stop", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "Stopping", body = JobInfo),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Already ended", body = crate::error::ErrorBody)))]
pub async fn stop(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<JobInfo>> {
    st.jobs.stop(&id).map(Json)
}

/// Removes a job (a running one is stopped first and recorded in the history).
#[utoipa::path(delete, path = "/api/jobs/{id}", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody)))]
pub async fn delete(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<StatusCode> {
    st.jobs.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Latest statistics snapshot of a job.
#[utoipa::path(get, path = "/api/jobs/{id}/stats", tag = "jobs",
    params(("id" = String, Path, description = "Job id")),
    responses(
        (status = 200, description = "`wpg_tx::job::StatsSnapshot`", body = Object),
        (status = 404, description = "Unknown id", body = crate::error::ErrorBody),
        (status = 409, description = "Never started", body = crate::error::ErrorBody)))]
pub async fn stats(
    State(st): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<Value>> {
    let s = st.jobs.stats(&id)?;
    serde_json::to_value(s)
        .map(Json)
        .map_err(ApiError::internal)
}

/// Finished jobs (last 100), newest first.
#[utoipa::path(get, path = "/api/jobs/history", tag = "jobs",
    responses((status = 200, description = "Finished jobs", body = Vec<JobInfo>)))]
pub async fn history(State(st): State<AppState>) -> Json<Vec<JobInfo>> {
    Json(st.jobs.history())
}

/// A running job on the dashboard.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct JobBrief {
    /// Job id.
    pub id: String,
    /// Name.
    pub name: String,
    /// State.
    pub status: JobStatus,
    /// Backend.
    pub backend: String,
    /// Interface.
    pub iface: String,
    /// Packets sent.
    pub packets: u64,
    /// Packets per second.
    pub pps: f64,
    /// L2 bits per second.
    pub bps: f64,
}

/// An interface on the dashboard.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IfaceBrief {
    /// Name.
    pub name: String,
    /// Administratively up.
    pub up: bool,
    /// Carrier / running.
    pub running: bool,
    /// Loopback.
    pub loopback: bool,
    /// Link speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<u64>,
    /// Allowed by the transmit policy.
    pub allowed: bool,
}

/// `GET /api/stats/overview` response.
#[derive(Debug, Serialize, ToSchema)]
pub struct Overview {
    /// Seconds since the server started.
    pub uptime_secs: u64,
    /// Totals over the active jobs.
    pub totals: JobTotals,
    /// Active jobs.
    pub jobs: Vec<JobBrief>,
    /// Interfaces.
    pub ifaces: Vec<IfaceBrief>,
    /// Templates in the library.
    pub templates: usize,
    /// Jobs in the history.
    pub history: usize,
}

/// Dashboard summary.
#[utoipa::path(get, path = "/api/stats/overview", tag = "jobs",
    responses((status = 200, description = "Dashboard data", body = Overview)))]
pub async fn overview(State(st): State<AppState>) -> ApiResult<Json<Overview>> {
    let jobs = st
        .jobs
        .list()
        .into_iter()
        .filter(|j| !j.status.is_terminal() && j.status != JobStatus::Created)
        .map(|j| {
            let (packets, pps, bps) = j
                .stats
                .as_ref()
                .map_or((0, 0.0, 0.0), |s| (s.packets, s.pps, s.bps));
            JobBrief {
                id: j.id,
                name: j.name,
                status: j.status,
                backend: j.backend,
                iface: j.iface,
                packets,
                pps,
                bps,
            }
        })
        .collect();
    let ifaces = tokio::task::spawn_blocking({
        let st = st.clone();
        move || st.ifaces(false).unwrap_or_default()
    })
    .await
    .map_err(ApiError::internal)?
    .into_iter()
    .map(|i| IfaceBrief {
        allowed: st.iface_allowed(&i.name),
        name: i.name,
        up: i.up,
        running: i.running,
        loopback: i.loopback,
        speed_mbps: i.speed_mbps,
    })
    .collect();
    Ok(Json(Overview {
        uptime_secs: st.started_at.elapsed().as_secs(),
        totals: st.jobs.totals(),
        jobs,
        ifaces,
        templates: st
            .templates
            .list(&crate::store::ListFilter::default())
            .len(),
        history: st.jobs.history().len(),
    }))
}
