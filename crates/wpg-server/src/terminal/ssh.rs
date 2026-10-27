//! The `russh` client side of a web terminal SSH session: host key checks
//! against [`super::known_hosts`] (asking the browser about unknown keys through
//! [`SshEvent`]s), the algorithm preferences and the session configuration. The
//! interactive parts (prompts, authentication order) live in
//! [`super::session`].

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use russh::client;
use russh::keys::PublicKeyOrCertificate;
use russh::{cipher, kex, mac, Preferred};
use tokio::sync::{mpsc, oneshot};

use super::known_hosts::{fingerprint, HostKeyStatus, KnownHost};
use super::Terminal;

/// The user's answer to a host key prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostKeyAnswer {
    /// Trust the key.
    pub accept: bool,
    /// Store it in the known hosts.
    pub save: bool,
}

/// What the handler (running inside `russh`'s session task) reports to the
/// WebSocket session.
#[derive(Debug)]
pub enum SshEvent {
    /// An unknown host key: ask the user, answer on `reply`.
    Prompt {
        /// Key algorithm.
        algorithm: String,
        /// `SHA256:...`.
        fingerprint: String,
        /// Where the answer goes.
        reply: oneshot::Sender<HostKeyAnswer>,
    },
    /// The host presented a different key than the stored one (refused).
    Changed {
        /// Algorithm of the presented key.
        algorithm: String,
        /// Fingerprint of the presented key.
        fingerprint: String,
        /// The stored key.
        known: KnownHost,
    },
    /// The host key was trusted (stored or just accepted): its fingerprint.
    Verified(String),
    /// The server's authentication banner.
    Banner(String),
}

/// `russh` client handler of one session.
#[derive(Debug)]
pub struct Client {
    terminal: Terminal,
    host: String,
    port: u16,
    /// wpg user (recorded in the known-hosts comment).
    user: String,
    events: mpsc::Sender<SshEvent>,
}

impl Client {
    /// A handler for `host`:`port` opened by wpg user `user`.
    #[must_use]
    pub fn new(
        terminal: Terminal,
        host: &str,
        port: u16,
        user: &str,
        events: mpsc::Sender<SshEvent>,
    ) -> Self {
        Self {
            terminal,
            host: host.to_owned(),
            port,
            user: user.to_owned(),
            events,
        }
    }
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = server_public_key.public_key();
        let fp = fingerprint(&key);
        let algorithm = key.algorithm().as_str().to_owned();
        match self
            .terminal
            .known_hosts()
            .check(&self.host, self.port, &key)
        {
            HostKeyStatus::Trusted => {
                let _ = self.events.try_send(SshEvent::Verified(fp));
                Ok(true)
            }
            HostKeyStatus::Changed(known) => {
                let _ = self
                    .events
                    .send(SshEvent::Changed {
                        algorithm,
                        fingerprint: fp,
                        known,
                    })
                    .await;
                Ok(false)
            }
            HostKeyStatus::Unknown => {
                let (reply, answer) = oneshot::channel();
                let prompt = SshEvent::Prompt {
                    algorithm,
                    fingerprint: fp.clone(),
                    reply,
                };
                if self.events.send(prompt).await.is_err() {
                    return Ok(false);
                }
                let Ok(a) = answer.await else {
                    return Ok(false);
                };
                if !a.accept {
                    return Ok(false);
                }
                if a.save {
                    let comment = format!(
                        "wpg:{}@{}",
                        self.user,
                        super::rfc3339(u64::try_from(crate::auth::now_secs()).unwrap_or(0))
                    );
                    match self
                        .terminal
                        .known_hosts()
                        .add(&self.host, self.port, &key, &comment)
                    {
                        Ok(()) => tracing::info!(
                            target: crate::auth::AUDIT,
                            user = %self.user,
                            host = %self.host,
                            port = self.port,
                            fingerprint = %fp,
                            "terminal host key stored"
                        ),
                        Err(e) => {
                            tracing::error!(
                                "{}: {e}",
                                self.terminal.known_hosts().path().display()
                            );
                        }
                    }
                }
                let _ = self.events.try_send(SshEvent::Verified(fp));
                Ok(true)
            }
        }
    }

    async fn auth_banner(
        &mut self,
        banner: &str,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let _ = self.events.try_send(SshEvent::Banner(banner.to_owned()));
        Ok(())
    }
}

/// Algorithm preferences: `russh`'s secure defaults first, then the older
/// algorithms network equipment still ships with (SHA-1 / NIST key exchange,
/// CBC ciphers, HMAC-SHA1), so a session to an old IOS image negotiates
/// something rather than nothing. The older ones are only chosen when the server
/// offers nothing better.
#[must_use]
pub fn preferred() -> Preferred {
    let d = Preferred::DEFAULT;
    let mut kex: Vec<kex::Name> = d.kex.to_vec();
    kex.extend([
        kex::ECDH_SHA2_NISTP256,
        kex::ECDH_SHA2_NISTP384,
        kex::ECDH_SHA2_NISTP521,
        kex::DH_G14_SHA1,
        kex::DH_GEX_SHA1,
        kex::DH_G1_SHA1,
    ]);
    let mut ciphers: Vec<cipher::Name> = d.cipher.to_vec();
    ciphers.extend([
        cipher::AES_128_GCM,
        cipher::AES_256_CBC,
        cipher::AES_192_CBC,
        cipher::AES_128_CBC,
    ]);
    let mut macs: Vec<mac::Name> = d.mac.to_vec();
    macs.extend([mac::HMAC_SHA1_ETM, mac::HMAC_SHA1]);
    Preferred {
        kex: Cow::Owned(kex),
        cipher: Cow::Owned(ciphers),
        mac: Cow::Owned(macs),
        ..d
    }
}

/// Client configuration: keep-alives every 30 s (3 unanswered end the session),
/// `TCP_NODELAY` for interactive typing. Idle sessions are closed by the
/// WebSocket side (`terminal.idle_timeout_secs`), not here.
#[must_use]
pub fn config() -> Arc<client::Config> {
    Arc::new(client::Config {
        preferred: preferred(),
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        inactivity_timeout: None,
        nodelay: true,
        ..client::Config::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_algorithms_come_first() {
        let p = preferred();
        assert_eq!(p.kex[0], Preferred::DEFAULT.kex[0]);
        assert!(p.kex.contains(&kex::DH_G14_SHA1));
        let pos = |n| p.cipher.iter().position(|c| *c == n).unwrap();
        assert!(pos(cipher::CHACHA20_POLY1305) < pos(cipher::AES_128_CBC));
        assert_eq!(p.mac.last(), Some(&mac::HMAC_SHA1));
    }
}
