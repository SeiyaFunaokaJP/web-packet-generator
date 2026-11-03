//! `wpg remote`: drive a running wpg server over its REST API with an API token
//! (`wpg config token create`, or the Settings page of the web UI).
//!
//! A small blocking client (`ureq`, rustls). Requests send
//! `Authorization: Bearer <token>` and `Accept-Language` (so server errors come
//! back in the CLI language); request and response bodies are the server's own
//! types (`JobSpec`, `JobInfo`, ...).
//!
//! Exit status: 0 success, 1 the job failed (`jobs wait`, `jobs start --wait`),
//! 2 any other error (unreachable server, rejected request, bad arguments).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde::de::DeserializeOwned;
use serde_json::Value;
use wpg_i18n::{t, Locale};
use wpg_server::api::templates::TemplateList;
use wpg_server::jobs::{JobInfo, JobSpec, JobStatus, JobStream};
use wpg_template::Template;

use crate::cmd::cli_locale;
use crate::cmd::send::tr;

/// Arguments of `wpg remote`.
#[derive(Debug, Args)]
pub(crate) struct RemoteArgs {
    /// Server URL.
    #[arg(
        long,
        env = "WPG_SERVER",
        default_value = "http://127.0.0.1:8080",
        global = true,
        value_name = "URL"
    )]
    pub(crate) server: String,
    /// API token (`wpg_...`). Visible in the process list: prefer `WPG_TOKEN`.
    #[arg(
        long,
        env = "WPG_TOKEN",
        hide_env_values = true,
        global = true,
        value_name = "TOKEN"
    )]
    pub(crate) token: Option<String>,
    /// Accept any TLS certificate (for servers with a self-signed certificate).
    #[arg(long, global = true)]
    pub(crate) insecure: bool,
    /// Print the server's JSON responses.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: RemoteCommand,
}

/// Subcommands of `wpg remote`.
#[derive(Debug, Subcommand)]
pub(crate) enum RemoteCommand {
    /// Transmit jobs: list, start, stop, wait.
    Jobs {
        #[command(subcommand)]
        action: JobsAction,
    },
    /// The server's template library: list, push.
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
    },
}

/// Actions of `wpg remote jobs`.
#[derive(Debug, Subcommand)]
pub(crate) enum JobsAction {
    /// List the server's jobs (running and recently finished).
    List,
    /// Start a job from a library template id or a local template file; prints
    /// the job id.
    Start(StartArgs),
    /// Stop a job.
    Stop {
        /// Job id.
        id: String,
    },
    /// Wait until a job ends; exit status 1 if it failed.
    Wait {
        /// Job id.
        id: String,
        /// Give up after this many seconds (exit status 2).
        #[arg(long, value_name = "SECS")]
        timeout: Option<u64>,
    },
}

/// Arguments of `wpg remote jobs start`.
#[derive(Debug, Args)]
pub(crate) struct StartArgs {
    /// Template id on the server, or a local `.wpg.yaml` file sent with the job.
    pub(crate) template: String,
    /// Template variable value (repeatable).
    #[arg(long = "var", value_name = "NAME=VALUE", value_parser = parse_var)]
    pub(crate) vars: Vec<(String, String)>,
    /// Rate, e.g. `1000pps`, `1Mpps`, `10Gbps`, `unlimited`.
    #[arg(long)]
    pub(crate) rate: Option<String>,
    /// Number of packets.
    #[arg(long)]
    pub(crate) count: Option<u64>,
    /// Duration limit, e.g. `10s`, `2m`.
    #[arg(long)]
    pub(crate) duration: Option<String>,
    /// Interface on the server.
    #[arg(short, long)]
    pub(crate) iface: Option<String>,
    /// Transmit backend (default: the server's).
    #[arg(long)]
    pub(crate) backend: Option<String>,
    /// Job name.
    #[arg(long)]
    pub(crate) name: Option<String>,
    /// Wait until the job ends (like `wpg remote jobs wait`).
    #[arg(long)]
    pub(crate) wait: bool,
}

/// Actions of `wpg remote templates`.
#[derive(Debug, Subcommand)]
pub(crate) enum TemplatesAction {
    /// List the templates on the server.
    List,
    /// Upload a template file to the server's user templates; prints its id.
    Push {
        /// Template file (`.wpg.yaml`).
        file: PathBuf,
    },
}

fn parse_var(s: &str) -> Result<(String, String), String> {
    match s.split_once('=') {
        Some((k, v)) if !k.trim().is_empty() => Ok((k.trim().to_owned(), v.to_owned())),
        _ => Err(format!("expected NAME=VALUE, got `{s}`")),
    }
}

/// Exit status of request errors.
const ERROR: u8 = 2;

/// A failed request.
#[derive(Debug)]
enum Failure {
    /// The server answered with an error.
    Http { status: u16, message: String },
    /// No answer (connection, TLS, timeout).
    Transport(String),
    /// The answer did not decode.
    Decode(String),
}

/// Percent-encodes a path segment.
fn enc(s: &str) -> String {
    use std::fmt::Write as _;
    s.bytes().fold(String::new(), |mut out, b| {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
        out
    })
}

/// The message of an error body (`{ "error": { "message" } }`), else the text.
fn error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| body.chars().take(300).collect())
}

struct Client {
    agent: ureq::Agent,
    base: String,
    token: Option<String>,
    locale: Locale,
}

impl Client {
    fn new(args: &RemoteArgs, locale: Locale) -> Result<Self, String> {
        let base = args.server.trim().trim_end_matches('/').to_owned();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(tr(locale, "remote-error-url", &[("url", base)]));
        }
        let tls = ureq::tls::TlsConfig::builder()
            .disable_verification(args.insecure)
            .build();
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(60)))
            .build()
            .new_agent();
        Ok(Self {
            agent,
            base,
            token: args.token.clone().filter(|t| !t.trim().is_empty()),
            locale,
        })
    }

    fn prepare<B>(&self, rb: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        let rb = rb
            .header("Accept", "application/json")
            .header("Accept-Language", self.locale.as_str());
        match &self.token {
            Some(t) => rb.header("Authorization", format!("Bearer {}", t.trim())),
            None => rb,
        }
    }

    fn finish(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<String, Failure> {
        let mut resp = r.map_err(|e| Failure::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .with_config()
            .limit(64 << 20)
            .read_to_string()
            .map_err(|e| Failure::Transport(e.to_string()))?;
        if (200..300).contains(&status) {
            Ok(body)
        } else {
            Err(Failure::Http {
                status,
                message: error_message(&body),
            })
        }
    }

    fn get(&self, path: &str) -> Result<String, Failure> {
        let url = format!("{}{path}", self.base);
        Self::finish(self.prepare(self.agent.get(&url)).call())
    }

    fn post(
        &self,
        path: &str,
        content_type: &str,
        body: Option<String>,
    ) -> Result<String, Failure> {
        let url = format!("{}{path}", self.base);
        let rb = self.prepare(self.agent.post(&url));
        Self::finish(match body {
            Some(b) => rb.header("Content-Type", content_type).send(b),
            None => rb.send_empty(),
        })
    }

    fn report(&self, f: &Failure) -> ExitCode {
        let l = self.locale;
        let msg = match f {
            Failure::Http { status, message } => tr(
                l,
                "remote-error-http",
                &[("status", status.to_string()), ("message", message.clone())],
            ),
            Failure::Transport(e) => tr(
                l,
                "remote-error-connect",
                &[("server", self.base.clone()), ("error", e.clone())],
            ),
            Failure::Decode(e) => tr(l, "remote-error-decode", &[("error", e.clone())]),
        };
        eprintln!("{msg}");
        ExitCode::from(ERROR)
    }
}

fn decode<T: DeserializeOwned>(body: &str) -> Result<T, Failure> {
    serde_json::from_str(body).map_err(|e| Failure::Decode(e.to_string()))
}

/// Pretty-prints a JSON response as received.
fn print_json(body: &str) {
    match serde_json::from_str::<Value>(body) {
        Ok(v) => println!("{v:#}"),
        Err(_) => println!("{body}"),
    }
}

/// Runs `wpg remote`.
pub(crate) fn run(args: &RemoteArgs) -> ExitCode {
    let locale = cli_locale();
    let client = match Client::new(args, locale) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(ERROR);
        }
    };
    let json = args.json;
    let result = match &args.command {
        RemoteCommand::Jobs { action } => match action {
            JobsAction::List => jobs_list(&client, json),
            JobsAction::Start(a) => jobs_start(&client, a, json),
            JobsAction::Stop { id } => jobs_stop(&client, id, json),
            JobsAction::Wait { id, timeout } => {
                jobs_wait(&client, id, timeout.map(Duration::from_secs), json)
            }
        },
        RemoteCommand::Templates { action } => match action {
            TemplatesAction::List => templates_list(&client, json),
            TemplatesAction::Push { file } => templates_push(&client, file, json),
        },
    };
    result.unwrap_or_else(|f| client.report(&f))
}

fn status_text(s: &JobStatus) -> String {
    match s {
        JobStatus::Finished(r) => format!(
            "finished ({})",
            serde_json::to_value(r)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        ),
        JobStatus::Failed(e) => format!("failed ({e})"),
        other => other.name().to_owned(),
    }
}

fn job_line(j: &JobInfo) -> String {
    let packets = j.stats.as_ref().map_or(0, |s| s.packets);
    let target = if j.iface.is_empty() {
        j.backend.clone()
    } else {
        format!("{}/{}", j.backend, j.iface)
    };
    format!(
        "{}  {}  {}  {target}  {packets} pkts",
        j.id,
        status_text(&j.status),
        j.name
    )
}

fn jobs_list(c: &Client, json: bool) -> Result<ExitCode, Failure> {
    let body = c.get("/api/jobs")?;
    if json {
        print_json(&body);
    } else {
        let jobs: Vec<JobInfo> = decode(&body)?;
        if jobs.is_empty() {
            println!("{}", t(c.locale, "remote-jobs-none"));
        }
        for j in &jobs {
            println!("{}", job_line(j));
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The job request of `wpg remote jobs start`.
fn start_spec(a: &StartArgs, locale: Locale) -> Result<JobSpec, String> {
    let path = Path::new(&a.template);
    let (template_id, template) = if path.is_file() {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let t = Template::from_yaml_str(&text).map_err(|e| {
            tr(
                locale,
                "remote-error-template",
                &[
                    ("file", path.display().to_string()),
                    ("error", e.to_string()),
                ],
            )
        })?;
        (None, Some(t))
    } else {
        (Some(a.template.clone()), None)
    };
    Ok(JobSpec {
        name: a.name.clone(),
        streams: vec![JobStream {
            template_id,
            template,
            vars: a.vars.iter().cloned().collect::<BTreeMap<_, _>>(),
            weight: 1,
        }],
        iface: a.iface.clone(),
        backend: a.backend.clone(),
        rate: a.rate.clone(),
        count: a.count,
        duration: a.duration.clone(),
        ..JobSpec::default()
    })
}

fn jobs_start(c: &Client, a: &StartArgs, json: bool) -> Result<ExitCode, Failure> {
    let spec = match start_spec(a, c.locale) {
        Ok(s) => s,
        Err(msg) => {
            eprintln!("{msg}");
            return Ok(ExitCode::from(ERROR));
        }
    };
    let mut req = serde_json::to_value(&spec).map_err(|e| Failure::Decode(e.to_string()))?;
    req["start"] = Value::Bool(true);
    let body = c.post("/api/jobs", "application/json", Some(req.to_string()))?;
    let job: JobInfo = decode(&body)?;
    if a.wait {
        if !json {
            eprintln!("{}", started(c.locale, &job));
        }
        return jobs_wait(c, &job.id, None, json);
    }
    if json {
        print_json(&body);
    } else {
        println!("{}", job.id);
        eprintln!("{}", started(c.locale, &job));
    }
    Ok(ExitCode::SUCCESS)
}

fn started(locale: Locale, job: &JobInfo) -> String {
    tr(
        locale,
        "remote-job-started",
        &[("id", job.id.clone()), ("name", job.name.clone())],
    )
}

fn jobs_stop(c: &Client, id: &str, json: bool) -> Result<ExitCode, Failure> {
    let body = c.post(&format!("/api/jobs/{}/stop", enc(id)), "", None)?;
    if json {
        print_json(&body);
    } else {
        println!(
            "{}",
            tr(c.locale, "remote-job-stopping", &[("id", id.to_owned())])
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// Exit status for an ended job: 1 when it failed.
fn exit_status(s: &JobStatus) -> u8 {
    u8::from(matches!(s, JobStatus::Failed(_)))
}

fn jobs_wait(
    c: &Client,
    id: &str,
    timeout: Option<Duration>,
    json: bool,
) -> Result<ExitCode, Failure> {
    let started = Instant::now();
    let path = format!("/api/jobs/{}", enc(id));
    loop {
        let body = c.get(&path)?;
        let job: JobInfo = decode(&body)?;
        if job.status.is_terminal() {
            if json {
                print_json(&body);
            } else {
                println!("{}", job_line(&job));
            }
            return Ok(ExitCode::from(exit_status(&job.status)));
        }
        if timeout.is_some_and(|t| started.elapsed() >= t) {
            eprintln!(
                "{}",
                tr(c.locale, "remote-wait-timeout", &[("id", id.to_owned())])
            );
            return Ok(ExitCode::from(ERROR));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn templates_list(c: &Client, json: bool) -> Result<ExitCode, Failure> {
    let body = c.get("/api/templates")?;
    if json {
        print_json(&body);
    } else {
        let list: TemplateList = decode(&body)?;
        for s in &list.templates {
            println!("{}  {}  {}", s.id, s.name, s.protocols.join(" / "));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn templates_push(c: &Client, file: &Path, json: bool) -> Result<ExitCode, Failure> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {e}", file.display());
            return Ok(ExitCode::from(ERROR));
        }
    };
    let body = c.post(
        "/api/templates/import",
        "text/yaml; charset=utf-8",
        Some(text),
    )?;
    if json {
        print_json(&body);
    } else {
        let v: Value = decode(&body)?;
        let id = v["summary"]["id"].as_str().unwrap_or_default().to_owned();
        println!("{id}");
        eprintln!(
            "{}",
            tr(
                c.locale,
                "remote-template-pushed",
                &[("file", file.display().to_string()), ("id", id)]
            )
        );
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(parse_var("a=1=2").unwrap(), ("a".into(), "1=2".into()));
        assert!(parse_var("=1").is_err());
        assert!(parse_var("x").is_err());
        assert_eq!(enc("a b/ü"), "a%20b%2F%C3%BC");
        assert_eq!(
            error_message(r#"{"error":{"code":"x","message":"Nope"}}"#),
            "Nope"
        );
        assert_eq!(error_message("plain"), "plain");
        assert_eq!(exit_status(&JobStatus::Failed("x".into())), 1);
        assert_eq!(
            exit_status(&JobStatus::Finished(wpg_server::jobs::EndReason::Stopped)),
            0
        );
        assert_eq!(
            status_text(&JobStatus::Finished(
                wpg_server::jobs::EndReason::CountReached
            )),
            "finished (count_reached)"
        );
    }

    #[test]
    fn spec_from_id_and_file() {
        let a = StartArgs {
            template: "examples-arp-sweep".into(),
            vars: vec![("n".into(), "3".into())],
            rate: Some("10pps".into()),
            count: Some(5),
            duration: None,
            iface: None,
            backend: Some("null".into()),
            name: None,
            wait: false,
        };
        let s = start_spec(&a, Locale::En).unwrap();
        assert_eq!(
            s.streams[0].template_id.as_deref(),
            Some("examples-arp-sweep")
        );
        assert_eq!(s.streams[0].vars["n"], "3");
        assert_eq!(s.count, Some(5));

        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.wpg.yaml");
        std::fs::write(&bad, "not: [a template").unwrap();
        let a = StartArgs {
            template: bad.display().to_string(),
            ..a
        };
        assert!(start_spec(&a, Locale::En)
            .unwrap_err()
            .contains("bad.wpg.yaml"));
    }
}
