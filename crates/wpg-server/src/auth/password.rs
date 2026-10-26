//! argon2id password hashing and the password sources (spec §2.6).
//!
//! Hashes are PHC strings (`$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`) with
//! the argon2 crate's defaults, which follow the OWASP recommendation (19 MiB,
//! 2 passes, 1 lane). Verification reads the parameters from the stored string
//! and compares in constant time.

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use thiserror::Error;

use crate::config::{read_secret_file, Config};

/// Shortest password the setup screen accepts.
pub const MIN_LEN: usize = 8;

/// Password errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PasswordError {
    /// Hashing failed (should not happen with the default parameters).
    #[error("password hashing failed: {0}")]
    Hash(String),
    /// A configured hash is not a valid argon2 PHC string.
    #[error("{source_name}: not a valid argon2 hash ({error})")]
    BadHash {
        /// Where it came from.
        source_name: &'static str,
        /// Parser message.
        error: String,
    },
    /// The password file cannot be read.
    #[error("{0}")]
    File(String),
}

/// Hashes `password` with argon2id and a random 128-bit salt.
///
/// # Errors
/// [`PasswordError::Hash`] when the OS random generator fails.
pub fn hash(password: &str) -> Result<String, PasswordError> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| PasswordError::Hash(e.to_string()))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| PasswordError::Hash(e.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| PasswordError::Hash(e.to_string()))
}

/// Parses an argon2 PHC string that carries a salt and a hash.
fn parse(phc: &str) -> Result<PasswordHash<'_>, String> {
    let h = PasswordHash::new(phc).map_err(|e| e.to_string())?;
    if !h.algorithm.as_str().starts_with("argon2") || h.salt.is_none() || h.hash.is_none() {
        return Err("incomplete argon2 PHC string".to_owned());
    }
    Ok(h)
}

/// Whether `password` matches the PHC string `phc` (constant-time comparison;
/// a malformed hash never matches).
#[must_use]
pub fn verify(password: &str, phc: &str) -> bool {
    parse(phc).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

/// Burns the time of one verification (unknown user names take as long as
/// wrong passwords).
pub fn dummy_verify(password: &str) {
    use std::sync::OnceLock;
    static DUMMY: OnceLock<String> = OnceLock::new();
    let h = DUMMY.get_or_init(|| hash("wpg-dummy-password").unwrap_or_default());
    let _ = verify(password, h);
}

/// Whether `s` parses as an argon2 PHC string.
#[must_use]
pub fn is_hash(s: &str) -> bool {
    parse(s).is_ok()
}

/// Hashes plain text, passes an argon2 hash through (validated).
fn external(value: &str, source: &'static str) -> Result<ExternalPassword, PasswordError> {
    if value.starts_with("$argon2") {
        parse(value).map_err(|error| PasswordError::BadHash {
            source_name: source,
            error,
        })?;
        Ok(ExternalPassword {
            hash: value.to_owned(),
            source,
            plain: None,
        })
    } else {
        Ok(ExternalPassword {
            hash: hash(value)?,
            source,
            plain: Some(value.to_owned()),
        })
    }
}

/// A password configured outside the database.
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalPassword {
    /// argon2 PHC string.
    pub hash: String,
    /// Where it came from: `command line / environment`, `auth.password_hash`,
    /// `auth.password_file`.
    pub source: &'static str,
    /// The plain text when the source was not already hashed (lets a restart
    /// keep the stored hash, and so the sessions, when the password is the same).
    pub plain: Option<String>,
}

impl std::fmt::Debug for ExternalPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalPassword")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

/// The admin password from the configuration, highest priority first:
/// `--password` / `--password-file` / `WPG_PASSWORD` (already merged into
/// `auth.password` by [`Config::load`]), `auth.password_hash`,
/// `auth.password_file`. Plain-text values are hashed here. `None`: the
/// database (setup screen) decides.
///
/// # Errors
/// Invalid hash, unreadable password file.
pub fn resolve(config: &Config) -> Result<Option<ExternalPassword>, PasswordError> {
    if let Some(p) = &config.auth.password {
        return external(p.expose(), "command line / environment").map(Some);
    }
    if let Some(h) = config
        .auth
        .password_hash
        .as_deref()
        .filter(|h| !h.is_empty())
    {
        const SRC: &str = "auth.password_hash";
        parse(h).map_err(|error| PasswordError::BadHash {
            source_name: SRC,
            error,
        })?;
        return Ok(Some(ExternalPassword {
            hash: h.to_owned(),
            source: SRC,
            plain: None,
        }));
    }
    if let Some(path) = &config.auth.password_file {
        const SRC: &str = "auth.password_file";
        let text = read_secret_file(path).map_err(|e| PasswordError::File(e.to_string()))?;
        return external(&text, SRC).map(Some);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Secret;

    #[test]
    fn hash_and_verify() {
        let h = hash("correct horse").unwrap();
        assert!(h.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"), "{h}");
        assert!(is_hash(&h));
        assert!(verify("correct horse", &h));
        assert!(!verify("correct horsE", &h));
        assert!(!verify("correct horse", "not a hash"));
        // salted: same password, different hash
        assert_ne!(h, hash("correct horse").unwrap());
        dummy_verify("x");
    }

    #[test]
    fn sources_in_priority_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Config::default();
        assert!(resolve(&c).unwrap().is_none());

        let file = dir.path().join("pw");
        std::fs::write(&file, "file-password\n").unwrap();
        c.auth.password_file = Some(file.clone());
        let r = resolve(&c).unwrap().unwrap();
        assert_eq!(r.source, "auth.password_file");
        assert!(verify("file-password", &r.hash));

        let stored = hash("hash-password").unwrap();
        std::fs::write(&file, &stored).unwrap();
        assert_eq!(resolve(&c).unwrap().unwrap().hash, stored);

        c.auth.password_hash = Some(hash("config-password").unwrap());
        let r = resolve(&c).unwrap().unwrap();
        assert_eq!(r.source, "auth.password_hash");
        assert!(verify("config-password", &r.hash));

        c.auth.password = Some(Secret::new("cli-password"));
        let r = resolve(&c).unwrap().unwrap();
        assert!(verify("cli-password", &r.hash));
        assert!(!format!("{r:?}").contains("argon2"));

        c.auth.password = None;
        c.auth.password_hash = Some("$argon2id$garbage".into());
        assert!(matches!(resolve(&c), Err(PasswordError::BadHash { .. })));
    }
}
