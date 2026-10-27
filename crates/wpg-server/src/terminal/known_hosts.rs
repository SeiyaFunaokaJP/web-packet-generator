//! SSH host keys trusted by the web terminal: `<data_dir>/terminal/known_hosts`.
//!
//! The file uses the OpenSSH `known_hosts` format (`host algorithm base64
//! comment`, `[host]:port` for ports other than 22), so it can be inspected,
//! pre-seeded or copied with the usual tools. Trust is on first use: an unknown
//! key is shown to the user, who confirms its fingerprint; a key that differs from
//! the stored one is refused until an administrator removes the old entry. Lines
//! wpg does not understand (hashed host names, markers) are kept as they are.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use russh::keys::{HashAlg, PublicKey};
use serde::Serialize;
use utoipa::ToSchema;

use super::target::normalize_host;

/// A stored host key (`GET /api/terminal/known-hosts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct KnownHost {
    /// Host as typed by the user who accepted the key (lower case).
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Key algorithm (`ssh-ed25519`, `rsa-sha2-512`, ...).
    pub algorithm: String,
    /// `SHA256:...` fingerprint.
    pub fingerprint: String,
    /// The entry's comment (who accepted it and when).
    pub comment: String,
}

/// Result of [`KnownHosts::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    /// The key is stored for this host and port.
    Trusted,
    /// No key is stored for this host and port.
    Unknown,
    /// A different key is stored: possible man-in-the-middle.
    Changed(KnownHost),
}

/// `SHA256:...` fingerprint of `key`.
#[must_use]
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// The `known_hosts` host field of `host`:`port`.
fn host_field(host: &str, port: u16) -> String {
    let host = normalize_host(host);
    if port == 22 {
        host
    } else {
        format!("[{host}]:{port}")
    }
}

/// Parses a host field (only plain `host` / `[host]:port`; no patterns or
/// hashes). Comma-separated lists yield every name.
fn parse_host_field(field: &str) -> Vec<(String, u16)> {
    field
        .split(',')
        .filter_map(|h| {
            if let Some(rest) = h.strip_prefix('[') {
                let (host, port) = rest.split_once("]:")?;
                Some((normalize_host(host), port.parse().ok()?))
            } else if h.starts_with('|') || h.contains(['*', '?', '!']) {
                None
            } else {
                Some((normalize_host(h), 22))
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct Entry {
    hosts: Vec<(String, u16)>,
    key: PublicKey,
}

#[derive(Debug, Clone)]
struct Line {
    raw: String,
    entry: Option<Entry>,
}

fn parse(text: &str) -> Vec<Line> {
    text.lines()
        .map(|raw| {
            let t = raw.trim();
            let entry = if t.is_empty() || t.starts_with('#') || t.starts_with('@') {
                None
            } else {
                t.split_once(char::is_whitespace).and_then(|(hosts, key)| {
                    let key = PublicKey::from_openssh(key.trim()).ok()?;
                    let hosts = parse_host_field(hosts);
                    (!hosts.is_empty()).then_some(Entry { hosts, key })
                })
            };
            Line {
                raw: raw.to_owned(),
                entry,
            }
        })
        .collect()
}

fn info(host: &str, port: u16, key: &PublicKey) -> KnownHost {
    KnownHost {
        host: host.to_owned(),
        port,
        algorithm: key.algorithm().as_str().to_owned(),
        fingerprint: fingerprint(key),
        comment: key.comment().to_string(),
    }
}

/// Removes `target` from the entries of `lines` (dropping entries left without
/// hosts). Returns whether any entry named it.
fn drop_host(lines: &mut Vec<Line>, target: &(String, u16)) -> bool {
    let mut removed = false;
    lines.retain_mut(|l| {
        let Some(e) = &l.entry else {
            return true;
        };
        if !e.hosts.contains(target) {
            return true;
        }
        removed = true;
        let rest: Vec<(String, u16)> = e.hosts.iter().filter(|h| *h != target).cloned().collect();
        if rest.is_empty() {
            return false;
        }
        let field = rest
            .iter()
            .map(|(h, p)| host_field(h, *p))
            .collect::<Vec<_>>()
            .join(",");
        let key = l
            .raw
            .trim()
            .split_once(char::is_whitespace)
            .map_or("", |(_, k)| k.trim());
        l.raw = format!("{field} {key}");
        if let Some(e) = &mut l.entry {
            e.hosts = rest;
        }
        true
    });
    removed
}

/// The known-hosts file (cheap to share behind an `Arc`; writes are serialised).
#[derive(Debug)]
pub struct KnownHosts {
    path: PathBuf,
    lock: Mutex<()>,
}

impl KnownHosts {
    /// The store at `path` (created on the first [`KnownHosts::add`]).
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> std::io::Result<Vec<Line>> {
        match std::fs::read_to_string(&self.path) {
            Ok(t) => Ok(parse(&t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    fn write(&self, lines: &[Line]) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::new();
        for l in lines {
            text.push_str(&l.raw);
            text.push('\n');
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }

    /// Compares `key` with the stored key of `host`:`port`. An unreadable file
    /// counts as empty (logged).
    #[must_use]
    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> HostKeyStatus {
        let _g = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let host = normalize_host(host);
        let lines = self.read().unwrap_or_else(|e| {
            tracing::warn!("{}: {e}", self.path.display());
            Vec::new()
        });
        let mut other = None;
        for e in lines.iter().filter_map(|l| l.entry.as_ref()) {
            if !e.hosts.iter().any(|(h, p)| *h == host && *p == port) {
                continue;
            }
            if e.key.key_data() == key.key_data() {
                return HostKeyStatus::Trusted;
            }
            other.get_or_insert_with(|| info(&host, port, &e.key));
        }
        other.map_or(HostKeyStatus::Unknown, HostKeyStatus::Changed)
    }

    /// Stores `key` for `host`:`port`, replacing what was stored for exactly that
    /// host and port; `comment` goes into the entry's comment field.
    ///
    /// # Errors
    /// I/O errors.
    pub fn add(
        &self,
        host: &str,
        port: u16,
        key: &PublicKey,
        comment: &str,
    ) -> std::io::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let host = normalize_host(host);
        let mut lines = self.read()?;
        drop_host(&mut lines, &(host.clone(), port));
        let mut key = key.clone();
        key.set_comment(
            comment
                .chars()
                .map(|c| if c.is_whitespace() { '_' } else { c })
                .collect::<String>(),
        );
        let text = key
            .to_openssh()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        lines.push(Line {
            raw: format!("{} {text}", host_field(&host, port)),
            entry: None,
        });
        self.write(&lines)
    }

    /// Every stored key wpg understands.
    ///
    /// # Errors
    /// I/O errors.
    pub fn list(&self) -> std::io::Result<Vec<KnownHost>> {
        let _g = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(self
            .read()?
            .iter()
            .filter_map(|l| l.entry.as_ref())
            .flat_map(|e| e.hosts.iter().map(|(h, p)| info(h, *p, &e.key)))
            .collect())
    }

    /// Removes every key of `host`:`port` (an entry listing several hosts loses
    /// only this one). Returns whether anything was removed.
    ///
    /// # Errors
    /// I/O errors.
    pub fn remove(&self, host: &str, port: u16) -> std::io::Result<bool> {
        let _g = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let host = normalize_host(host);
        let mut lines = self.read()?;
        let removed = drop_host(&mut lines, &(host, port));
        if removed {
            self.write(&lines)?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
    use russh::keys::PrivateKey;

    fn key(seed: u8) -> PublicKey {
        let kp = Ed25519Keypair::from_seed(&[seed; 32]);
        PrivateKey::new(KeypairData::Ed25519(kp), "")
            .unwrap()
            .public_key()
            .clone()
    }

    #[test]
    fn trust_on_first_use() {
        let dir = tempfile::tempdir().unwrap();
        let kh = KnownHosts::new(dir.path().join("t/known_hosts"));
        let (k1, k2) = (key(1), key(2));
        assert_eq!(kh.check("R1", 22, &k1), HostKeyStatus::Unknown);
        kh.add("R1", 22, &k1, "added-by admin").unwrap();
        assert_eq!(kh.check("r1", 22, &k1), HostKeyStatus::Trusted);
        // another port is another host
        assert_eq!(kh.check("r1", 2222, &k1), HostKeyStatus::Unknown);
        // a different key is refused and the stored one is described
        let HostKeyStatus::Changed(old) = kh.check("r1", 22, &k2) else {
            panic!("expected a changed key");
        };
        assert_eq!(old.fingerprint, fingerprint(&k1));
        assert_eq!(old.algorithm, "ssh-ed25519");
        assert_eq!(old.comment, "added-by_admin");
        assert!(fingerprint(&k1).starts_with("SHA256:"));

        kh.add("r1", 2222, &k2, "x").unwrap();
        let text = std::fs::read_to_string(kh.path()).unwrap();
        assert!(text.starts_with("r1 ssh-ed25519 AAAA"), "{text}");
        assert!(text.contains("\n[r1]:2222 ssh-ed25519 "), "{text}");
        assert_eq!(kh.list().unwrap().len(), 2);

        // replacing keeps one entry per host and port
        assert!(kh.remove("r1", 22).unwrap());
        assert!(!kh.remove("r1", 22).unwrap());
        kh.add("r1", 22, &k2, "y").unwrap();
        assert_eq!(kh.check("r1", 22, &k2), HostKeyStatus::Trusted);
        assert_eq!(kh.list().unwrap().len(), 2);
    }

    #[test]
    fn foreign_lines_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        let k = key(3);
        let line = k.to_openssh().unwrap();
        std::fs::write(
            &path,
            format!("# comment\n|1|abc=|def= {line}\nsw1,sw2,[sw3]:830 {line}\nbroken line\n"),
        )
        .unwrap();
        let kh = KnownHosts::new(path.clone());
        assert_eq!(kh.check("sw2", 22, &k), HostKeyStatus::Trusted);
        assert_eq!(kh.check("sw3", 830, &k), HostKeyStatus::Trusted);
        assert_eq!(kh.list().unwrap().len(), 3);
        assert!(kh.remove("sw2", 22).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# comment\n|1|abc=|def= "), "{text}");
        assert!(text.contains("\nsw1,[sw3]:830 ssh-ed25519 "), "{text}");
        assert!(text.contains("broken line"), "{text}");
        assert_eq!(kh.check("sw2", 22, &k), HostKeyStatus::Unknown);
        assert_eq!(kh.check("sw1", 22, &k), HostKeyStatus::Trusted);
    }
}
