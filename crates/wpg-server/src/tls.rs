//! HTTPS (TODO §2.6 "TLS: 自己署名自動生成 or 証明書指定 (`rustls`)").
//!
//! * `[server.tls] cert/key`: PEM files given by the operator.
//! * `server.self_signed_tls` / `wpg serve --tls`: a self-signed ECDSA P-256
//!   certificate is generated with `rcgen` into `<data_dir>/tls/{cert,key}.pem`
//!   and reused on later starts (regenerated when it approaches its expiry: the
//!   validity is kept under Apple's 825-day limit). Subject alternative names:
//!   `localhost`, `127.0.0.1`, `::1`, the bind address (unless unspecified) and
//!   the host name. The SHA-256 fingerprint is logged so that users can check
//!   what the browser shows.
//!
//! rustls runs on the `ring` provider (no cmake / NASM needed to build).

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustls::pki_types::pem::{self, PemObject};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::Config;

/// Validity of generated certificates.
pub const SELF_SIGNED_DAYS: u64 = 800;
/// Generated certificates older than this are replaced.
const REGENERATE_AFTER_DAYS: u64 = SELF_SIGNED_DAYS - 30;

/// TLS setup errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TlsError {
    /// Reading / writing a PEM file.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// No certificate / key in a PEM file.
    #[error("{0}: no {1} found in the PEM file")]
    Empty(String, &'static str),
    /// Certificate generation failed.
    #[error("certificate generation failed: {0}")]
    Generate(String),
    /// rustls rejected the certificate / key.
    #[error("TLS configuration: {0}")]
    Rustls(String),
}

/// The server's TLS configuration.
#[derive(Debug, Clone)]
pub struct TlsSetup {
    /// For the acceptor.
    pub config: Arc<rustls::ServerConfig>,
    /// SHA-256 of the leaf certificate, `AA:BB:...`.
    pub fingerprint: String,
    /// Certificate file.
    pub cert_path: PathBuf,
    /// Self-signed certificate in `<data_dir>/tls`.
    pub self_signed: bool,
    /// Generated during this start.
    pub generated: bool,
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> TlsError + '_ {
    move |source| TlsError::Io {
        path: path.display().to_string(),
        source,
    }
}

fn pem_err(path: &Path) -> impl FnOnce(pem::Error) -> TlsError + '_ {
    move |e| io_err(path)(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// The TLS configuration for `config`, `None` when TLS is off.
///
/// # Errors
/// Unreadable / invalid PEM files, generation failures.
pub fn setup(config: &Config) -> Result<Option<TlsSetup>, TlsError> {
    if let Some(t) = &config.server.tls {
        return load(&t.cert, &t.key, false, false).map(Some);
    }
    if !config.server.self_signed_tls {
        return Ok(None);
    }
    let dir = config.tls_dir();
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    let fresh = cert.is_file()
        && key.is_file()
        && std::fs::metadata(&cert)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < Duration::from_secs(REGENERATE_AFTER_DAYS * 86_400));
    let generated = if fresh {
        false
    } else {
        generate(&dir, config.server.bind)?;
        true
    };
    load(&cert, &key, true, generated).map(Some)
}

fn load(cert: &Path, key: &Path, self_signed: bool, generated: bool) -> Result<TlsSetup, TlsError> {
    let cert_pem = std::fs::read(cert).map_err(io_err(cert))?;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&cert_pem)
        .collect::<Result<_, _>>()
        .map_err(pem_err(cert))?;
    if certs.is_empty() {
        return Err(TlsError::Empty(cert.display().to_string(), "certificate"));
    }
    let key_pem = std::fs::read(key).map_err(io_err(key))?;
    let key_der = match PrivateKeyDer::from_pem_slice(&key_pem) {
        Ok(k) => k,
        Err(pem::Error::NoItemsFound) => {
            return Err(TlsError::Empty(key.display().to_string(), "private key"));
        }
        Err(e) => return Err(pem_err(key)(e)),
    };
    let fingerprint = fingerprint(&certs[0]);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsError::Rustls(e.to_string()))?
        .with_no_client_auth()
        .with_single_cert(certs, key_der)
        .map_err(|e| TlsError::Rustls(e.to_string()))?;
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(TlsSetup {
        config: Arc::new(cfg),
        fingerprint,
        cert_path: cert.to_path_buf(),
        self_signed,
        generated,
    })
}

/// `AA:BB:...` SHA-256 of a DER certificate.
#[must_use]
pub fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The host name of this machine, if known.
fn hostname() -> Option<String> {
    ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| {
            !h.is_empty()
                && h.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        })
}

/// Subject alternative names for a certificate serving `bind`.
#[must_use]
pub fn subject_alt_names(bind: SocketAddr) -> Vec<String> {
    let mut names = vec![
        "localhost".to_owned(),
        "127.0.0.1".to_owned(),
        "::1".to_owned(),
    ];
    let ip: IpAddr = bind.ip();
    if !ip.is_unspecified() && !ip.is_loopback() {
        names.push(ip.to_string());
    }
    if let Some(h) = hostname() {
        if !names.contains(&h) {
            names.push(h);
        }
    }
    names
}

/// Civil date (UTC) of Unix `secs`.
fn ymd(secs: u64) -> (i32, u8, u8) {
    // Howard Hinnant's days-to-civil algorithm.
    let z = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        i32::try_from(y).unwrap_or(2000),
        u8::try_from(m).unwrap_or(1),
        u8::try_from(d).unwrap_or(1),
    )
}

/// Generates a self-signed certificate + key into `dir` (key file mode 0600).
///
/// # Errors
/// Generation / write failures.
pub fn generate(dir: &Path, bind: SocketAddr) -> Result<(), TlsError> {
    let gen = |e: rcgen::Error| TlsError::Generate(e.to_string());
    let mut params = rcgen::CertificateParams::new(subject_alt_names(bind)).map_err(gen)?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "wpg self-signed certificate");
    params
        .distinguished_name
        .push(rcgen::DnType::OrganizationName, "Web Packet Generator");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = ymd(now.saturating_sub(86_400));
    params.not_before = rcgen::date_time_ymd(y, m, d);
    let (y, m, d) = ymd(now + SELF_SIGNED_DAYS * 86_400);
    params.not_after = rcgen::date_time_ymd(y, m, d);
    let key = rcgen::KeyPair::generate().map_err(gen)?;
    let cert = params.self_signed(&key).map_err(gen)?;

    std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    write_private(&key_path, key.serialize_pem().as_bytes()).map_err(io_err(&key_path))?;
    std::fs::write(&cert_path, cert.pem()).map_err(io_err(&cert_path))?;
    Ok(())
}

fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    opts.open(path)?.write_all(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(ymd(0), (1970, 1, 1));
        assert_eq!(ymd(951_782_400), (2000, 2, 29));
        assert_eq!(ymd(1_790_000_000), (2026, 9, 21));
    }

    #[test]
    fn self_signed_is_generated_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Config {
            data_dir: dir.path().to_path_buf(),
            ..Config::default()
        };
        assert!(setup(&c).unwrap().is_none());
        c.server.self_signed_tls = true;
        c.server.bind = "192.0.2.7:8443".parse().unwrap();
        let a = setup(&c).unwrap().unwrap();
        assert!(a.generated && a.self_signed);
        assert_eq!(a.fingerprint.len(), 32 * 3 - 1);
        let b = setup(&c).unwrap().unwrap();
        assert!(!b.generated);
        assert_eq!(a.fingerprint, b.fingerprint);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.path().join("tls/key.pem"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(subject_alt_names(c.server.bind).contains(&"192.0.2.7".to_owned()));

        // explicit files win
        c.server.tls = Some(crate::config::TlsConfig {
            cert: dir.path().join("tls/cert.pem"),
            key: dir.path().join("tls/key.pem"),
        });
        let e = setup(&c).unwrap().unwrap();
        assert!(!e.self_signed);
        assert_eq!(e.fingerprint, a.fingerprint);
        c.server.tls = Some(crate::config::TlsConfig {
            cert: dir.path().join("nope.pem"),
            key: dir.path().join("tls/key.pem"),
        });
        assert!(matches!(setup(&c), Err(TlsError::Io { .. })));
    }
}
