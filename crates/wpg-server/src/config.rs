//! Server configuration: `wpg.toml` → `WPG_*` environment → command line
//! (TODO §2.8 "設定ファイル `wpg.toml` のフル例と `wpg config init`").
//!
//! Precedence, lowest first:
//!
//! 1. built-in defaults ([`Config::default`]);
//! 2. the TOML file (`wpg.toml` in the working directory, or `--config <path>`);
//! 3. environment variables `WPG_<SECTION>__<KEY>` (`__` separates nesting levels,
//!    e.g. `WPG_SERVER__BIND=0.0.0.0:8080`, `WPG_TX__ALLOWED_IFACES=[eth1,eth2]`,
//!    `WPG_DATA_DIR=/var/lib/wpg`);
//! 4. command line overrides ([`Overrides`]).
//!
//! Unknown keys are errors (typos in `wpg.toml` must not silently fall back to a
//! default).
//!
//! The admin password is not a regular key: `wpg serve --password`,
//! `--password-file` and the `WPG_PASSWORD` environment variable (in that order)
//! land in [`AuthConfig::password`], which is never serialised. Below them come
//! `auth.password_hash`, `auth.password_file` and finally the user created on the
//! first-run setup screen (see [`crate::auth`]).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use figment::providers::{Env, Format, Serialized, Toml};
use figment::Figment;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use wpg_log::{LogConfig, LogFormat, Rotation};

/// Default configuration file name, looked up in the working directory.
pub const DEFAULT_CONFIG_FILE: &str = "wpg.toml";

/// Environment variable prefix (`WPG_SERVER__BIND`, ...).
pub const ENV_PREFIX: &str = "WPG_";

/// Environment variable holding the admin password (plain text or argon2 hash).
pub const PASSWORD_ENV: &str = "WPG_PASSWORD";

/// Replacement text for secrets in [`Config::to_toml_redacted`].
pub const REDACTED: &str = "<redacted>";

/// Top-level sections a `WPG_*` variable may address. Other `WPG_*` variables
/// (`WPG_BIND`, `WPG_CONFIG` of the CLI) are not configuration keys.
const ENV_SECTIONS: &[&str] = &["server", "auth", "templates", "data_dir", "tx", "log"];

/// The whole configuration (`wpg.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Directory for server state: user templates, favourites, the `SQLite`
    /// database (`wpg.sqlite`: users, sessions, job history) and the
    /// self-signed TLS certificate. Default: the OS data directory
    /// (`~/.local/share/wpg`, `%APPDATA%\wpg\data`, `~/Library/Application Support/wpg`)
    /// or `./data` when that cannot be determined.
    pub data_dir: PathBuf,
    /// HTTP listener.
    pub server: ServerConfig,
    /// Authentication.
    pub auth: AuthConfig,
    /// Template library.
    pub templates: TemplatesConfig,
    /// Transmit policy and defaults.
    pub tx: TxConfig,
    /// Logging.
    pub log: LogConfig,
}

/// `[server]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Listen address. Without a password (and no user created on the setup
    /// screen) only loopback addresses are accepted (`--allow-insecure` overrides).
    pub bind: SocketAddr,
    /// TLS certificate / key (PEM). Takes precedence over `self_signed_tls`.
    pub tls: Option<TlsConfig>,
    /// Serve HTTPS with a self-signed certificate generated into
    /// `<data_dir>/tls/` (reused on later starts). `wpg serve --tls` sets it.
    pub self_signed_tls: bool,
    /// Public URL of the server when it sits behind a reverse proxy
    /// (`https://wpg.example.net`), used for absolute links.
    pub base_url: Option<String>,
}

/// `[server.tls]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsConfig {
    /// PEM certificate chain.
    pub cert: PathBuf,
    /// PEM private key.
    pub key: PathBuf,
}

/// `[auth]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    /// argon2id PHC string of the admin password (`$argon2id$v=19$...`);
    /// `wpg config set-password` writes it.
    pub password_hash: Option<String>,
    /// File holding the admin password: an argon2 PHC string, or plain text that
    /// is hashed at startup (alternative to `password_hash`).
    pub password_file: Option<PathBuf>,
    /// Session lifetime in seconds (sliding: every request extends it).
    pub session_ttl_secs: u64,
    /// Password from the command line / `WPG_PASSWORD` (plain text or argon2
    /// hash). Never read from or written to `wpg.toml`.
    #[serde(skip)]
    pub password: Option<Secret>,
}

/// A secret string whose `Debug` output is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps `s`.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The secret.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(REDACTED)
    }
}

/// `[templates]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TemplatesConfig {
    /// Read-only template libraries (e.g. Git checkouts), scanned recursively for
    /// `*.wpg.yaml` / `*.wpg.json`.
    pub library_paths: Vec<PathBuf>,
    /// Writable directory for templates created in the GUI. Default:
    /// `<data_dir>/templates` (see [`Config::user_template_dir`]).
    pub user_dir: Option<PathBuf>,
}

/// `[tx]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TxConfig {
    /// Interfaces jobs may transmit on; empty = all (TODO §2.6 "送出先インタフェース
    /// 許可リスト"). The `null` backend and dry runs are not restricted.
    pub allowed_ifaces: Vec<String>,
    /// Highest rate a job may request (`10kpps`, `1Gbps`); `None` = no limit. Jobs
    /// without a rate get this rate.
    pub max_rate: Option<String>,
    /// Backend used when a job does not name one (`None` = the build's default:
    /// `afpacket` on Linux, else `pcap`, else `null`).
    pub default_backend: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            tls: None,
            self_signed_tls: false,
            base_url: None,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            password_hash: None,
            password_file: None,
            session_ttl_secs: 24 * 60 * 60,
            password: None,
        }
    }
}

impl Default for TemplatesConfig {
    fn default() -> Self {
        Self {
            library_paths: vec![PathBuf::from("./templates")],
            user_dir: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            templates: TemplatesConfig::default(),
            tx: TxConfig::default(),
            log: LogConfig::default(),
        }
    }
}

/// The OS data directory for wpg, or `./data`.
#[must_use]
pub fn default_data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "wpg")
        .map_or_else(|| PathBuf::from("./data"), |d| d.data_dir().to_path_buf())
}

/// Command line overrides (highest precedence). `None` / empty = not given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// `--password` (visible in the process list: prefer the others).
    pub password: Option<Secret>,
    /// `--password-file`: plain text or argon2 hash.
    pub password_file: Option<PathBuf>,
    /// `--tls`: serve HTTPS with a self-signed certificate.
    pub tls: bool,
    /// `--bind`.
    pub bind: Option<SocketAddr>,
    /// `--library-path` (repeatable; replaces the configured list).
    pub library_paths: Vec<PathBuf>,
    /// `--data-dir`.
    pub data_dir: Option<PathBuf>,
}

/// Configuration loading errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// An explicitly requested config file does not exist.
    #[error("config file {0} does not exist")]
    Missing(String),
    /// Syntax / type / unknown key error (names the source and key).
    #[error("{0}")]
    Invalid(Box<figment::Error>),
    /// Writing a file failed.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
}

impl From<figment::Error> for ConfigError {
    fn from(e: figment::Error) -> Self {
        ConfigError::Invalid(Box::new(e))
    }
}

/// Severity of a [`ConfigIssue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueLevel {
    /// The server will not work as configured.
    Error,
    /// Works, but probably not as intended.
    Warning,
}

/// A finding of [`Config::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigIssue {
    /// Severity.
    pub level: IssueLevel,
    /// Dotted key, e.g. `tx.max_rate`.
    pub key: String,
    /// Fluent message key (`config-issue-*`).
    pub message_key: String,
    /// Offending value / detail (untranslated).
    pub detail: String,
}

impl ConfigIssue {
    fn new(level: IssueLevel, key: &str, message_key: &str, detail: impl Into<String>) -> Self {
        Self {
            level,
            key: key.to_owned(),
            message_key: message_key.to_owned(),
            detail: detail.into(),
        }
    }
}

impl Config {
    /// The figment for `file` (see the module docs for the precedence). `file`:
    /// `Some(path)` must exist; `None` reads `wpg.toml` if present.
    ///
    /// # Errors
    /// [`ConfigError::Missing`] for an explicit file that does not exist.
    pub fn figment(file: Option<&Path>, overrides: &Overrides) -> Result<Figment, ConfigError> {
        let mut fig = Figment::from(Serialized::defaults(Config::default()));
        match file {
            Some(path) if !path.exists() => {
                return Err(ConfigError::Missing(path.display().to_string()))
            }
            Some(path) => fig = fig.merge(Toml::file_exact(path)),
            None if Path::new(DEFAULT_CONFIG_FILE).is_file() => {
                fig = fig.merge(Toml::file_exact(DEFAULT_CONFIG_FILE));
            }
            None => {}
        }
        fig = fig.merge(env_provider());
        if let Some(bind) = overrides.bind {
            fig = fig.merge(("server.bind", bind.to_string()));
        }
        if !overrides.library_paths.is_empty() {
            fig = fig.merge(("templates.library_paths", &overrides.library_paths));
        }
        if let Some(dir) = &overrides.data_dir {
            fig = fig.merge(("data_dir", dir));
        }
        if overrides.tls {
            fig = fig.merge(("server.self_signed_tls", true));
        }
        Ok(fig)
    }

    /// Loads the configuration (see [`Config::figment`]) and resolves the
    /// command line / environment password into [`AuthConfig::password`]:
    /// `--password`, then `--password-file`, then `WPG_PASSWORD`.
    ///
    /// # Errors
    /// Missing explicit file, TOML syntax errors, wrong types or unknown keys, an
    /// unreadable / empty `--password-file`.
    pub fn load(file: Option<&Path>, overrides: &Overrides) -> Result<Config, ConfigError> {
        let mut c: Config = Self::figment(file, overrides)?.extract()?;
        c.auth.password = if let Some(p) = &overrides.password {
            Some(p.clone())
        } else if let Some(path) = &overrides.password_file {
            Some(Secret::new(read_secret_file(path)?))
        } else {
            std::env::var(PASSWORD_ENV)
                .ok()
                .filter(|p| !p.is_empty())
                .map(Secret::new)
        };
        Ok(c)
    }

    /// Parses a TOML document on top of the defaults (no environment, no file).
    ///
    /// # Errors
    /// Syntax / type / unknown key errors.
    pub fn from_toml_str(text: &str) -> Result<Config, ConfigError> {
        Ok(Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::string(text))
            .extract()?)
    }

    /// Writable template directory: `templates.user_dir` or `<data_dir>/templates`.
    #[must_use]
    pub fn user_template_dir(&self) -> PathBuf {
        self.templates
            .user_dir
            .clone()
            .unwrap_or_else(|| self.data_dir.join("templates"))
    }

    /// Directory of the JSON job records of wpg ≤ M3 (imported into the database
    /// on first start, then renamed to `history.imported`).
    #[must_use]
    pub fn history_dir(&self) -> PathBuf {
        self.data_dir.join("history")
    }

    /// Directory dry-run captures (`pcapfile` backend) are written to.
    #[must_use]
    pub fn capture_dir(&self) -> PathBuf {
        self.data_dir.join("captures")
    }

    /// Whether an admin password is configured outside the database (command
    /// line, environment, `auth.password_hash` / `auth.password_file`).
    #[must_use]
    pub fn has_password(&self) -> bool {
        self.auth.password.is_some()
            || self
                .auth
                .password_hash
                .as_deref()
                .is_some_and(|h| !h.is_empty())
            || self.auth.password_file.is_some()
    }

    /// The `SQLite` database (users, sessions, login attempts, job history).
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("wpg.sqlite")
    }

    /// Directory of the generated self-signed certificate.
    #[must_use]
    pub fn tls_dir(&self) -> PathBuf {
        self.data_dir.join("tls")
    }

    /// Default backend name (configured or the build's default).
    #[must_use]
    pub fn default_backend(&self) -> String {
        self.tx
            .default_backend
            .clone()
            .unwrap_or_else(|| wpg_tx::backend::default_name().to_owned())
    }

    /// The effective configuration as TOML with secrets replaced by
    /// [`REDACTED`] and derived defaults (`templates.user_dir`) filled in.
    ///
    /// # Errors
    /// Only if TOML serialisation fails (non-UTF-8 paths).
    pub fn to_toml_redacted(&self) -> Result<String, toml::ser::Error> {
        let mut c = self.clone();
        if c.auth.password_hash.is_some() {
            c.auth.password_hash = Some(REDACTED.to_owned());
        }
        c.templates.user_dir = Some(self.user_template_dir());
        toml::to_string_pretty(&c)
    }

    /// Semantic checks beyond parsing (file existence, rate syntax, backend names,
    /// exposure without a password).
    #[must_use]
    pub fn validate(&self) -> Vec<ConfigIssue> {
        use IssueLevel::{Error, Warning};
        let mut out = Vec::new();
        if let Some(tls) = &self.server.tls {
            for (key, p) in [("server.tls.cert", &tls.cert), ("server.tls.key", &tls.key)] {
                if !p.is_file() {
                    out.push(ConfigIssue::new(
                        Error,
                        key,
                        "config-issue-file-missing",
                        p.display().to_string(),
                    ));
                }
            }
        }
        if let Some(url) = &self.server.base_url {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                out.push(ConfigIssue::new(
                    Error,
                    "server.base_url",
                    "config-issue-bad-url",
                    url.clone(),
                ));
            }
        }
        if let Some(h) = &self.auth.password_hash {
            if !h.starts_with("$argon2") {
                out.push(ConfigIssue::new(
                    Warning,
                    "auth.password_hash",
                    "config-issue-not-argon2",
                    "",
                ));
            }
            if self.auth.password_file.is_some() {
                out.push(ConfigIssue::new(
                    Warning,
                    "auth.password_file",
                    "config-issue-password-both",
                    "",
                ));
            }
        }
        if let Some(p) = &self.auth.password_file {
            if !p.is_file() {
                out.push(ConfigIssue::new(
                    Error,
                    "auth.password_file",
                    "config-issue-file-missing",
                    p.display().to_string(),
                ));
            }
        }
        if self.auth.session_ttl_secs == 0 {
            out.push(ConfigIssue::new(
                Error,
                "auth.session_ttl_secs",
                "config-issue-zero",
                "0",
            ));
        }
        if !self.has_password() && !self.server.bind.ip().is_loopback() {
            out.push(ConfigIssue::new(
                Warning,
                "server.bind",
                "config-issue-exposed-without-password",
                self.server.bind.to_string(),
            ));
        }
        for p in &self.templates.library_paths {
            if !p.is_dir() {
                out.push(ConfigIssue::new(
                    Warning,
                    "templates.library_paths",
                    "config-issue-dir-missing",
                    p.display().to_string(),
                ));
            }
        }
        if let Some(r) = &self.tx.max_rate {
            match wpg_tx::rate::Rate::parse(r) {
                Ok(rate) if rate.is_unlimited() => {}
                Ok(wpg_tx::rate::Rate::LinkPercent { .. }) => out.push(ConfigIssue::new(
                    Error,
                    "tx.max_rate",
                    "config-issue-max-rate-percent",
                    r.clone(),
                )),
                Ok(_) => {}
                Err(e) => out.push(ConfigIssue::new(
                    Error,
                    "tx.max_rate",
                    "config-issue-bad-rate",
                    e.to_string(),
                )),
            }
        }
        if let Some(b) = &self.tx.default_backend {
            match wpg_tx::backend::available()
                .into_iter()
                .find(|i| &i.name == b)
            {
                None => out.push(ConfigIssue::new(
                    Error,
                    "tx.default_backend",
                    "config-issue-unknown-backend",
                    b.clone(),
                )),
                Some(i) if !i.available => out.push(ConfigIssue::new(
                    Warning,
                    "tx.default_backend",
                    "config-issue-backend-unavailable",
                    format!("{b}: {}", i.reason.unwrap_or_default()),
                )),
                Some(_) => {}
            }
        }
        if let Err(e) = tracing_subscriber::EnvFilter::try_new(&self.log.level) {
            out.push(ConfigIssue::new(
                Error,
                "log.level",
                "config-issue-bad-log-level",
                e.to_string(),
            ));
        }
        out
    }

    /// A fully commented example `wpg.toml` (`wpg config init`). Every active
    /// line equals the built-in default, so the file changes nothing until edited.
    #[must_use]
    pub fn example_toml() -> &'static str {
        EXAMPLE_TOML
    }
}

/// Sets `auth.password_hash` in the TOML file `path` (created from
/// [`Config::example_toml`] when missing), keeping the rest of the file and its
/// comments. The result is checked to still parse.
///
/// # Errors
/// I/O errors, a file that is not valid TOML / configuration.
pub fn write_password_hash(path: &Path, hash: &str) -> Result<(), ConfigError> {
    let io = |source| ConfigError::Io {
        path: path.display().to_string(),
        source,
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => EXAMPLE_TOML.to_owned(),
        Err(e) => return Err(io(e)),
    };
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| {
        io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;
    if !doc.contains_table("auth") {
        doc["auth"] = toml_edit::table();
    }
    doc["auth"]["password_hash"] = toml_edit::value(hash);
    let out = doc.to_string();
    Config::from_toml_str(&out)?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, out).map_err(io)
}

/// Reads a password file: trimmed, must not be empty.
///
/// # Errors
/// [`ConfigError::Io`] when unreadable or empty.
pub fn read_secret_file(path: &Path) -> Result<String, ConfigError> {
    let io = |source| ConfigError::Io {
        path: path.display().to_string(),
        source,
    };
    let text = std::fs::read_to_string(path).map_err(io)?;
    let text = text.trim();
    if text.is_empty() {
        return Err(io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "empty password file",
        )));
    }
    Ok(text.to_owned())
}

fn env_provider() -> Env {
    Env::prefixed(ENV_PREFIX).split("__").filter(|k| {
        let k = k.as_str();
        let section = k.split('.').next().unwrap_or(k);
        ENV_SECTIONS.iter().any(|s| s.eq_ignore_ascii_case(section))
    })
}

const EXAMPLE_TOML: &str = r#"# wpg.toml — Web Packet Generator configuration.
#
# Precedence (lowest first): built-in defaults < this file < environment variables
# (WPG_<SECTION>__<KEY>, e.g. WPG_SERVER__BIND=0.0.0.0:8080 or
# WPG_TX__ALLOWED_IFACES='["eth1"]') < command line options (wpg serve --bind ...).
# Unknown keys are rejected. Check a file with `wpg config validate`, print the
# effective configuration with `wpg config show`.

# Server state: user templates, favourites, job history, dry-run captures.
# Default: the OS data directory (Linux: ~/.local/share/wpg).
# data_dir = "/var/lib/wpg"

[server]
# Listen address. Without a password keep it on localhost.
bind = "127.0.0.1:8080"
# Public URL when running behind a reverse proxy.
# base_url = "https://wpg.example.net"
# Serve HTTPS with a self-signed certificate generated into <data_dir>/tls/
# (same as `wpg serve --tls`). Ignored when [server.tls] is set.
self_signed_tls = false

# TLS certificate chain and private key (PEM) to serve HTTPS with.
# [server.tls]
# cert = "/etc/wpg/cert.pem"
# key = "/etc/wpg/key.pem"

[auth]
# Admin password. Without one (and without a user created on the first-run
# setup screen) the server only listens on localhost. Sources, highest first:
# `wpg serve --password`, `--password-file`, $WPG_PASSWORD, password_hash,
# password_file, the setup screen.
# argon2id hash of the admin password (`wpg config set-password` writes it):
# password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
# Or a file holding the hash or the plain-text password:
# password_file = "/etc/wpg/password"
# Session lifetime in seconds (extended by every request).
session_ttl_secs = 86400

[templates]
# Read-only template libraries (Git checkouts etc.), scanned recursively for
# *.wpg.yaml / *.wpg.json and reloaded when files change.
library_paths = ["./templates"]
# Writable directory for templates created or imported in the GUI.
# Default: <data_dir>/templates
# user_dir = "/var/lib/wpg/templates"

[tx]
# Interfaces jobs may send on; empty = all. The null backend and dry runs are
# not restricted.
allowed_ifaces = []
# Highest rate a job may request, e.g. "100kpps" or "1Gbps". Jobs without a rate
# are sent at this rate.
# max_rate = "100kpps"
# Backend when a job names none: null, pcapfile, afpacket, pcap, afxdp, dpdk.
# Default: afpacket on Linux, else pcap, else null.
# default_backend = "afpacket"

[log]
# Filter directives (RUST_LOG overrides this), e.g. "info" or "wpg_tx=debug,info".
level = "info"
# "pretty" or "json".
format = "pretty"
# Also write to this file (its directory is created).
# file = "/var/log/wpg/wpg.log"
# Rotation of the file: "never", "hourly" or "daily".
rotation = "never"
# Keep at most this many rotated files.
# max_files = 7
# Colour the console output when it is a terminal.
ansi = true
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn uncomment_all(text: &str) -> String {
        text.lines()
            .map(|l| {
                let t = l.trim_start();
                // `# key = value` and `# [section]` lines (not prose comments).
                match t.strip_prefix("# ") {
                    Some(rest)
                        if rest.starts_with('[')
                            || rest
                                .split_once(" = ")
                                .is_some_and(|(k, _)| !k.contains(' ')) =>
                    {
                        rest.to_owned()
                    }
                    _ => l.to_owned(),
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn example_equals_defaults() {
        assert_eq!(
            Config::from_toml_str(Config::example_toml()).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn example_with_everything_uncommented_parses() {
        let text = uncomment_all(Config::example_toml());
        let c = Config::from_toml_str(&text).unwrap();
        assert_eq!(c.data_dir, PathBuf::from("/var/lib/wpg"));
        assert_eq!(c.tx.max_rate.as_deref(), Some("100kpps"));
        assert_eq!(c.log.max_files, Some(7));
        assert!(c.server.tls.is_some());
        assert!(c.auth.password_file.is_some());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let e = Config::from_toml_str("[server]\nbindd = \"1.2.3.4:1\"\n").unwrap_err();
        assert!(e.to_string().contains("bindd"), "{e}");
        let e = Config::from_toml_str("[nope]\nx = 1\n").unwrap_err();
        assert!(e.to_string().contains("nope"), "{e}");
    }

    #[test]
    fn redacted_show_hides_secrets() {
        let mut c = Config::default();
        c.auth.password_hash = Some("$argon2id$secret".into());
        let s = c.to_toml_redacted().unwrap();
        assert!(!s.contains("secret"), "{s}");
        assert!(s.contains(REDACTED));
        // and it parses back
        let back = Config::from_toml_str(&s).unwrap();
        assert_eq!(back.auth.password_hash.as_deref(), Some(REDACTED));
        assert_eq!(back.templates.user_dir, Some(c.user_template_dir()));
    }

    #[test]
    #[allow(clippy::result_large_err)] // figment's Jail API
    fn precedence_file_env_cli() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "wpg.toml",
                r#"
                data_dir = "/from/file"
                [server]
                bind = "127.0.0.1:1111"
                [tx]
                max_rate = "1kpps"
                allowed_ifaces = ["eth0"]
                "#,
            )?;
            // file only
            let c = Config::load(None, &Overrides::default()).unwrap();
            assert_eq!(c.server.bind, "127.0.0.1:1111".parse().unwrap());
            assert_eq!(c.data_dir, PathBuf::from("/from/file"));
            assert_eq!(c.tx.allowed_ifaces, vec!["eth0".to_owned()]);

            // env beats file
            jail.set_env("WPG_SERVER__BIND", "127.0.0.1:2222");
            jail.set_env("WPG_TX__MAX_RATE", "2kpps");
            jail.set_env("WPG_TX__ALLOWED_IFACES", "[eth1, eth2]");
            jail.set_env("WPG_LOG__FORMAT", "json");
            // not configuration keys: ignored rather than rejected
            jail.set_env("WPG_BIND", "127.0.0.1:9");
            jail.set_env("WPG_CONFIG", "other.toml");
            let c = Config::load(None, &Overrides::default()).unwrap();
            assert_eq!(c.server.bind, "127.0.0.1:2222".parse().unwrap());
            assert_eq!(c.tx.max_rate.as_deref(), Some("2kpps"));
            assert_eq!(
                c.tx.allowed_ifaces,
                vec!["eth1".to_owned(), "eth2".to_owned()]
            );
            assert_eq!(c.log.format, LogFormat::Json);
            assert_eq!(c.data_dir, PathBuf::from("/from/file"));

            // CLI beats env
            let o = Overrides {
                bind: Some("127.0.0.1:3333".parse().unwrap()),
                library_paths: vec!["/lib/a".into(), "/lib/b".into()],
                data_dir: Some("/from/cli".into()),
                tls: true,
                ..Overrides::default()
            };
            let c = Config::load(None, &o).unwrap();
            assert_eq!(c.server.bind, "127.0.0.1:3333".parse().unwrap());
            assert_eq!(
                c.templates.library_paths,
                vec![PathBuf::from("/lib/a"), PathBuf::from("/lib/b")]
            );
            assert_eq!(c.data_dir, PathBuf::from("/from/cli"));
            assert!(c.server.self_signed_tls);
            assert_eq!(c.user_template_dir(), PathBuf::from("/from/cli/templates"));
            // untouched keys keep their file / default values
            assert_eq!(c.tx.max_rate.as_deref(), Some("2kpps"));
            assert_eq!(c.auth.session_ttl_secs, 86400);
            Ok(())
        });
    }

    #[test]
    fn explicit_missing_file_is_an_error() {
        let e = Config::load(
            Some(Path::new("/definitely/not/here/wpg.toml")),
            &Overrides::default(),
        )
        .unwrap_err();
        assert!(matches!(e, ConfigError::Missing(_)));
    }

    #[test]
    fn validation_findings() {
        let mut c = Config::default();
        c.templates.library_paths = vec![];
        assert!(c.validate().is_empty(), "{:?}", c.validate());
        c.tx.max_rate = Some("fast".into());
        c.tx.default_backend = Some("warp".into());
        c.server.bind = "0.0.0.0:8080".parse().unwrap();
        c.log.level = "info,[".into();
        let keys: Vec<(IssueLevel, String)> =
            c.validate().into_iter().map(|i| (i.level, i.key)).collect();
        assert!(keys.contains(&(IssueLevel::Error, "tx.max_rate".into())));
        assert!(keys.contains(&(IssueLevel::Error, "tx.default_backend".into())));
        assert!(keys.contains(&(IssueLevel::Warning, "server.bind".into())));
        assert!(keys.contains(&(IssueLevel::Error, "log.level".into())));
    }
}

#[cfg(test)]
mod password_source_tests {
    use super::*;

    #[test]
    fn write_password_hash_keeps_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/wpg.toml");
        write_password_hash(&path, "$argon2id$v=19$m=8,t=1,p=1$c2FsdHNhbHQ$aGFzaA").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# wpg.toml"), "comments kept");
        let c = Config::from_toml_str(&text).unwrap();
        assert!(c.auth.password_hash.unwrap().starts_with("$argon2id$"));
        // replaced, not duplicated
        write_password_hash(&path, "$argon2id$second").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("password_hash ="))
                .count(),
            1,
            "{text}"
        );
        assert!(text.contains("$argon2id$second"));
        // a file without [auth]
        let p2 = dir.path().join("min.toml");
        std::fs::write(&p2, "[server]\nbind = \"127.0.0.1:1\"\n").unwrap();
        write_password_hash(&p2, "$argon2id$x").unwrap();
        let c = Config::from_toml_str(&std::fs::read_to_string(&p2).unwrap()).unwrap();
        assert_eq!(c.auth.password_hash.as_deref(), Some("$argon2id$x"));
        assert_eq!(c.server.bind, "127.0.0.1:1".parse().unwrap());
    }

    #[test]
    #[allow(clippy::result_large_err)] // figment's Jail API
    fn password_sources_in_priority_order() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("pw.txt", "  from-file \n")?;
            jail.create_file("empty.txt", " \n")?;
            let c = Config::load(None, &Overrides::default()).unwrap();
            assert!(c.auth.password.is_none());
            assert!(!c.has_password());

            jail.set_env(PASSWORD_ENV, "from-env");
            let c = Config::load(None, &Overrides::default()).unwrap();
            assert_eq!(c.auth.password.as_ref().unwrap().expose(), "from-env");
            assert!(c.has_password());
            // never shown
            assert!(!format!("{c:?}").contains("from-env"));
            assert!(!c.to_toml_redacted().unwrap().contains("from-env"));

            let o = Overrides {
                password_file: Some("pw.txt".into()),
                ..Overrides::default()
            };
            let c = Config::load(None, &o).unwrap();
            assert_eq!(c.auth.password.as_ref().unwrap().expose(), "from-file");

            let o = Overrides {
                password: Some(Secret::new("from-cli")),
                password_file: Some("pw.txt".into()),
                ..Overrides::default()
            };
            let c = Config::load(None, &o).unwrap();
            assert_eq!(c.auth.password.as_ref().unwrap().expose(), "from-cli");

            let o = Overrides {
                password_file: Some("empty.txt".into()),
                ..Overrides::default()
            };
            assert!(matches!(
                Config::load(None, &o),
                Err(ConfigError::Io { .. })
            ));
            Ok(())
        });
    }
}
