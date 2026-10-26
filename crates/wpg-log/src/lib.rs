//! # wpg-log
//!
//! Logging setup shared by the wpg binaries (spec §2.8): a serialisable
//! [`LogConfig`] (level, pretty / JSON format, optional rolling log file, ANSI
//! colours), [`init()`] which installs the global `tracing` subscriber, and
//! [`layer_with_sink`], a `tracing` layer that forwards every event to a
//! `tokio::sync::broadcast` channel (used by the server's live log stream).
//!
//! `RUST_LOG` always overrides [`LogConfig::level`]. When a [`LogConfig::file`] is
//! configured, events are written to it *in addition to* the console, through a
//! non-blocking writer; keep the returned [`LogGuard`] alive until shutdown so that
//! buffered lines are flushed.

use std::fmt;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation as AppenderRotation};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Console / file line format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    /// Human readable, one line per event.
    #[default]
    Pretty,
    /// One JSON object per line.
    Json,
}

/// How often the log file is rotated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    /// A single file, never rotated.
    #[default]
    Never,
    /// A new file every hour (`name.YYYY-MM-DD-HH`).
    Hourly,
    /// A new file every day (`name.YYYY-MM-DD`).
    Daily,
}

impl From<Rotation> for AppenderRotation {
    fn from(r: Rotation) -> Self {
        match r {
            Rotation::Never => AppenderRotation::NEVER,
            Rotation::Hourly => AppenderRotation::HOURLY,
            Rotation::Daily => AppenderRotation::DAILY,
        }
    }
}

/// Logging configuration (the `log` section of the server config file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// `EnvFilter` directives, e.g. `info` or `wpg_tx=debug,info`.
    pub level: String,
    /// Line format.
    pub format: LogFormat,
    /// Log file (its directory is created); `None` = console only.
    pub file: Option<PathBuf>,
    /// Rotation of `file`.
    pub rotation: Rotation,
    /// Keep at most this many rotated files (oldest are deleted).
    pub max_files: Option<usize>,
    /// Colour the console output when it is a terminal.
    pub ansi: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            format: LogFormat::Pretty,
            file: None,
            rotation: Rotation::Never,
            max_files: None,
            ansi: true,
        }
    }
}

/// Errors of [`init()`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LogError {
    /// `level` is not a valid filter directive list.
    #[error("invalid log level `{level}`: {reason}")]
    Level {
        /// The offending value.
        level: String,
        /// Parser message.
        reason: String,
    },
    /// The log file could not be opened.
    #[error("cannot open log file {path}: {reason}")]
    File {
        /// The path.
        path: String,
        /// Why.
        reason: String,
    },
    /// A global subscriber was installed already.
    #[error("a global tracing subscriber is already installed")]
    AlreadyInitialized,
}

/// Keeps the non-blocking file writer alive; dropping it flushes pending lines.
#[derive(Debug)]
pub struct LogGuard {
    _file: Option<WorkerGuard>,
}

type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync + 'static>;

/// The filter: `RUST_LOG` if set and valid, else `level`.
///
/// # Errors
/// [`LogError::Level`] if `level` cannot be parsed (and `RUST_LOG` is unusable).
pub fn env_filter(level: &str) -> Result<EnvFilter, LogError> {
    EnvFilter::try_from_default_env().or_else(|_| {
        EnvFilter::try_new(level).map_err(|e| LogError::Level {
            level: level.to_owned(),
            reason: e.to_string(),
        })
    })
}

/// A `fmt` layer writing to `writer` in the given format.
fn fmt_layer<S, W>(format: LogFormat, ansi: bool, writer: W) -> BoxedLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + Send + Sync + 'static,
{
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(ansi);
    match format {
        LogFormat::Pretty => layer.boxed(),
        LogFormat::Json => layer.json().boxed(),
    }
}

/// Opens the (rolling) log file behind a non-blocking writer.
fn file_writer(cfg: &LogConfig, path: &Path) -> Result<(RollingFileAppender, String), LogError> {
    let fail = |reason: String| LogError::File {
        path: path.display().to_string(),
        reason,
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| fail("the path has no file name".into()))?
        .to_owned();
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir).map_err(|e| fail(e.to_string()))?;
    let mut builder = RollingFileAppender::builder()
        .rotation(cfg.rotation.into())
        .filename_prefix(&name);
    if let Some(n) = cfg.max_files.filter(|n| *n > 0) {
        builder = builder.max_log_files(n);
    }
    let appender = builder.build(dir).map_err(|e| fail(e.to_string()))?;
    Ok((appender, name))
}

/// Installs the global subscriber described by `cfg`.
///
/// # Errors
/// Invalid `level`, an unusable log file, or an already installed subscriber.
pub fn init(cfg: &LogConfig) -> Result<LogGuard, LogError> {
    init_with_sink(cfg, None)
}

/// [`init()`] plus an optional broadcast sink (see [`layer_with_sink`]).
///
/// # Errors
/// See [`init()`].
pub fn init_with_sink(
    cfg: &LogConfig,
    sink: Option<broadcast::Sender<LogEvent>>,
) -> Result<LogGuard, LogError> {
    let filter = env_filter(&cfg.level)?;
    let mut layers: Vec<BoxedLayer<tracing_subscriber::Registry>> = Vec::new();
    let ansi = cfg.ansi && std::io::stdout().is_terminal();
    layers.push(fmt_layer(cfg.format, ansi, std::io::stdout));
    let mut guard = None;
    if let Some(path) = &cfg.file {
        let (appender, _) = file_writer(cfg, path)?;
        let (writer, g) = tracing_appender::non_blocking(appender);
        guard = Some(g);
        layers.push(fmt_layer(cfg.format, false, writer));
    }
    if let Some(tx) = sink {
        layers.push(layer_with_sink(tx).boxed());
    }
    tracing_subscriber::registry()
        .with(layers)
        .with(filter)
        .try_init()
        .map_err(|_| LogError::AlreadyInitialized)?;
    Ok(LogGuard { _file: guard })
}

/// One log event as forwarded to a sink (and serialised to WebSocket clients).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    /// RFC 3339 UTC timestamp with millisecond precision.
    pub ts: String,
    /// `TRACE`, `DEBUG`, `INFO`, `WARN` or `ERROR`.
    pub level: String,
    /// Event target (usually the module path).
    pub target: String,
    /// The formatted `message` field.
    pub message: String,
    /// All other event fields.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// A `tracing` layer that sends every event to a broadcast channel. Send errors
/// (no receivers) are ignored.
pub struct SinkLayer {
    tx: broadcast::Sender<LogEvent>,
}

impl fmt::Debug for SinkLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SinkLayer")
            .field("receivers", &self.tx.receiver_count())
            .finish()
    }
}

/// Creates the layer forwarding events to `tx`; compose it with
/// `tracing_subscriber::registry().with(...)`.
#[must_use]
pub fn layer_with_sink(tx: broadcast::Sender<LogEvent>) -> SinkLayer {
    SinkLayer { tx }
}

#[derive(Default)]
struct Collect {
    message: String,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl Visit for Collect {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.insert(
                field.name().to_owned(),
                serde_json::Value::String(format!("{value:?}")),
            );
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            value.clone_into(&mut self.message);
        } else {
            self.fields
                .insert(field.name().to_owned(), value.to_owned().into());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_owned(), value.into());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields.insert(field.name().to_owned(), value.into());
    }
}

impl<S: Subscriber> Layer<S> for SinkLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if self.tx.receiver_count() == 0 {
            return;
        }
        let mut c = Collect::default();
        event.record(&mut c);
        let meta = event.metadata();
        let _ = self.tx.send(LogEvent {
            ts: now_rfc3339(),
            level: meta.level().as_str().to_owned(),
            target: meta.target().to_owned(),
            message: c.message,
            fields: c.fields,
        });
    }
}

/// `2026-09-29T12:34:56.789Z` for the current time.
fn now_rfc3339() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    rfc3339(d.as_secs(), d.subsec_millis())
}

/// Formats Unix time as RFC 3339 UTC (civil-from-days, Howard Hinnant).
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn rfc3339(secs: u64, millis: u32) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buf {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buf {
        type Writer = Buf;
        fn make_writer(&'a self) -> Buf {
            self.clone()
        }
    }

    #[test]
    fn config_defaults_and_serde() {
        let c = LogConfig::default();
        assert_eq!(c.level, "info");
        assert_eq!(c.format, LogFormat::Pretty);
        assert!(c.file.is_none() && c.max_files.is_none() && c.ansi);
        let parsed: LogConfig = serde_json::from_str(
            r#"{"level":"debug","format":"json","file":"/tmp/x.log","rotation":"daily","max_files":7}"#,
        )
        .unwrap();
        assert_eq!(parsed.format, LogFormat::Json);
        assert_eq!(parsed.rotation, Rotation::Daily);
        assert_eq!(parsed.max_files, Some(7));
        assert_eq!(
            serde_json::from_str::<LogConfig>("{}").unwrap(),
            LogConfig::default()
        );
        assert!(serde_json::from_str::<LogConfig>(r#"{"bogus":1}"#).is_err());
    }

    #[test]
    fn json_lines_parse() {
        let buf = Buf::default();
        let sub =
            tracing_subscriber::registry().with(fmt_layer(LogFormat::Json, false, buf.clone()));
        tracing::subscriber::with_default(sub, || {
            tracing::info!(port = 8080, "listening");
            tracing::warn!("second");
        });
        let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert_eq!(lines[0]["level"], "INFO");
        assert_eq!(lines[0]["fields"]["message"], "listening");
        assert_eq!(lines[0]["fields"]["port"], 8080);
        assert_eq!(lines[1]["level"], "WARN");
        assert!(lines[0]["timestamp"].is_string());
    }

    #[test]
    fn pretty_lines_are_plain_text() {
        let buf = Buf::default();
        let sub =
            tracing_subscriber::registry().with(fmt_layer(LogFormat::Pretty, false, buf.clone()));
        tracing::subscriber::with_default(sub, || tracing::error!("boom"));
        let text = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
        assert!(text.contains("ERROR") && text.contains("boom"), "{text}");
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn rotation_config_builds_appenders() {
        let dir = std::env::temp_dir().join(format!("wpg-log-test-{}", std::process::id()));
        for (rotation, max_files) in [
            (Rotation::Never, None),
            (Rotation::Hourly, Some(3)),
            (Rotation::Daily, Some(7)),
        ] {
            let cfg = LogConfig {
                file: Some(dir.join("nested").join("wpg.log")),
                rotation,
                max_files,
                ..LogConfig::default()
            };
            let (mut appender, name) = file_writer(&cfg, cfg.file.as_ref().unwrap()).unwrap();
            assert_eq!(name, "wpg.log");
            io::Write::write_all(&mut appender, b"hello\n").unwrap();
            io::Write::flush(&mut appender).unwrap();
        }
        let names: Vec<String> = std::fs::read_dir(dir.join("nested"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().any(|n| n == "wpg.log"), "{names:?}");
        assert!(names.iter().any(|n| n.starts_with("wpg.log.")), "{names:?}");
        std::fs::remove_dir_all(&dir).unwrap();

        let bad = LogConfig {
            file: Some(PathBuf::from("/")),
            ..LogConfig::default()
        };
        assert!(matches!(
            file_writer(&bad, Path::new("/")),
            Err(LogError::File { .. })
        ));
    }

    #[test]
    fn invalid_level_is_reported() {
        // RUST_LOG is unset in tests; a broken directive must not panic.
        if std::env::var_os("RUST_LOG").is_none() {
            assert!(matches!(env_filter("info,=="), Err(LogError::Level { .. })));
            assert!(env_filter("wpg_tx=debug,info").is_ok());
        }
    }

    #[test]
    fn sink_layer_receives_events() {
        let (tx, mut rx) = broadcast::channel(16);
        let sub = tracing_subscriber::registry().with(layer_with_sink(tx));
        tracing::subscriber::with_default(sub, || {
            tracing::info!(target: "wpg::test", count = 3u64, ok = true, name = "x", "hello {}", 1);
            tracing::debug!("second");
        });
        let e = rx.try_recv().unwrap();
        assert_eq!(e.level, "INFO");
        assert_eq!(e.target, "wpg::test");
        assert_eq!(e.message, "hello 1");
        assert_eq!(e.fields["count"], 3);
        assert_eq!(e.fields["ok"], true);
        assert_eq!(e.fields["name"], "x");
        assert!(e.ts.ends_with('Z') && e.ts.len() == 24, "{}", e.ts);
        assert_eq!(rx.try_recv().unwrap().level, "DEBUG");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn rfc3339_known_dates() {
        assert_eq!(rfc3339(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(1_782_734_096, 7), "2026-06-29T11:54:56.007Z");
        assert_eq!(rfc3339(951_782_400, 999), "2000-02-29T00:00:00.999Z");
    }
}
