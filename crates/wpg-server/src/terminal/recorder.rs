//! Session logs (like Tera Term's "Log..."): what the remote side sent, written to
//! `<data_dir>/terminal-logs/<UTC time>_<protocol>_<host>_<port>_<id>.log`.
//!
//! Only the output of the remote side is recorded, never the keystrokes (which
//! would include passwords typed at a prompt). By default escape sequences are
//! removed ([`AnsiStripper`], Tera Term's "Plain text"); `strip_ansi: false` keeps
//! the raw stream, which `cat` replays with colours.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

/// A log file stops growing at this size (the session continues unrecorded).
pub const MAX_LOG_BYTES: u64 = 256 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AState {
    #[default]
    Text,
    /// After a `CR` (dropped unless followed by `LF`).
    Cr,
    Esc,
    /// `ESC [` ... final byte 0x40..=0x7e.
    Csi,
    /// `ESC ]` ... `BEL` or `ESC \`.
    Osc,
    OscEsc,
    /// `ESC P`, `ESC X`, `ESC ^`, `ESC _` ... `ESC \`.
    Str,
    StrEsc,
    /// `ESC (`, `ESC )`, `ESC #`, ...: one more byte.
    Charset,
}

/// Removes terminal control sequences from a byte stream (state kept across
/// chunks): CSI / OSC / DCS sequences, other escapes, C0 controls except `LF`
/// and `TAB`; `CR LF` becomes `LF`, a lone `CR` is dropped.
#[derive(Debug, Clone, Default)]
pub struct AnsiStripper {
    state: AState,
}

impl AnsiStripper {
    /// Appends the printable text of `input` to `out`.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) {
        for &b in input {
            self.state = match self.state {
                // A lone CR (a redrawn line) simply vanishes: what follows is kept.
                AState::Text | AState::Cr => match b {
                    0x1b => AState::Esc,
                    b'\r' => AState::Cr,
                    b'\n' | b'\t' => {
                        out.push(b);
                        AState::Text
                    }
                    0x00..=0x1f | 0x7f => AState::Text,
                    _ => {
                        out.push(b);
                        AState::Text
                    }
                },
                AState::Esc => match b {
                    b'[' => AState::Csi,
                    b']' => AState::Osc,
                    b'P' | b'X' | b'^' | b'_' => AState::Str,
                    b'(' | b')' | b'*' | b'+' | b'-' | b'.' | b'/' | b'#' | b'%' | b' ' => {
                        AState::Charset
                    }
                    _ => AState::Text,
                },
                AState::Csi => match b {
                    0x40..=0x7e | 0x18 | 0x1a => AState::Text,
                    0x1b => AState::Esc,
                    _ => AState::Csi,
                },
                AState::Osc => match b {
                    0x07 => AState::Text,
                    0x1b => AState::OscEsc,
                    _ => AState::Osc,
                },
                AState::OscEsc | AState::StrEsc if b == b'\\' => AState::Text,
                AState::OscEsc => AState::Osc,
                AState::Str => {
                    if b == 0x1b {
                        AState::StrEsc
                    } else {
                        AState::Str
                    }
                }
                AState::StrEsc => AState::Str,
                AState::Charset => AState::Text,
            };
        }
    }

    /// Convenience: strips one complete buffer.
    #[must_use]
    pub fn strip(input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        Self::default().feed(input, &mut out);
        out
    }
}

/// A past or running session log (`GET /api/terminal/logs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LogInfo {
    /// File name (the id in `/api/terminal/logs/{name}`).
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Last modification (Unix seconds).
    pub modified: u64,
    /// Still being written by a live session.
    pub active: bool,
}

/// Whether `name` can name a log file (no paths, `.log` suffix).
#[must_use]
pub fn valid_name(name: &str) -> bool {
    name.len() <= 200
        && name.ends_with(".log")
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// File name of a new log.
#[must_use]
pub fn file_name(now_secs: u64, protocol: &str, host: &str, port: u16, id: &str) -> String {
    let (date, time) = super::utc_parts(now_secs);
    format!(
        "{}-{}_{}_{}_{port}_{}.log",
        date.replace('-', ""),
        time.replace(':', ""),
        sanitize(protocol),
        sanitize(host),
        sanitize(id)
    )
}

/// An open session log.
#[derive(Debug)]
pub struct Recorder {
    file: tokio::fs::File,
    name: String,
    strip: Option<AnsiStripper>,
    written: u64,
    buf: Vec<u8>,
}

impl Recorder {
    /// Creates `dir/name` (and `dir`; mode 0600 on Unix: logs may hold secrets
    /// printed by the remote side).
    ///
    /// # Errors
    /// I/O errors.
    pub async fn create(dir: &Path, name: String, strip_ansi: bool) -> std::io::Result<Self> {
        tokio::fs::create_dir_all(dir).await?;
        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        opts.mode(0o600);
        let file = opts.open(dir.join(&name)).await?;
        Ok(Self {
            file,
            name,
            strip: strip_ansi.then(AnsiStripper::default),
            written: 0,
            buf: Vec::new(),
        })
    }

    /// The file name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Appends remote output. `Ok(false)` once the size limit is reached (the
    /// recorder should be dropped).
    ///
    /// # Errors
    /// I/O errors.
    pub async fn write(&mut self, data: &[u8]) -> std::io::Result<bool> {
        let bytes: &[u8] = match &mut self.strip {
            Some(s) => {
                self.buf.clear();
                s.feed(data, &mut self.buf);
                &self.buf
            }
            None => data,
        };
        let room = MAX_LOG_BYTES.saturating_sub(self.written);
        let n = usize::try_from(room).unwrap_or(usize::MAX).min(bytes.len());
        self.file.write_all(&bytes[..n]).await?;
        self.written += n as u64;
        Ok(self.written < MAX_LOG_BYTES)
    }

    /// Flushes and closes the file.
    pub async fn finish(mut self) {
        let _ = self.file.flush().await;
        let _ = self.file.sync_all().await;
    }
}

/// Lists the logs in `dir` (newest first); `active` names files still written.
///
/// # Errors
/// I/O errors other than a missing directory.
pub fn list(dir: &Path, active: &[String]) -> std::io::Result<Vec<LogInfo>> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in rd {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !valid_name(&name) {
            continue;
        }
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        out.push(LogInfo {
            active: active.contains(&name),
            name,
            size: meta.len(),
            modified,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Path of log `name` in `dir`, if the name is valid.
#[must_use]
pub fn path_of(dir: &Path, name: &str) -> Option<PathBuf> {
    valid_name(name).then(|| dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escape_sequences() {
        let s = |b: &[u8]| String::from_utf8(AnsiStripper::strip(b)).unwrap();
        assert_eq!(s(b"\x1b[1;31mred\x1b[0m plain\r\n"), "red plain\n");
        assert_eq!(s(b"\x1b]0;title\x07after"), "after");
        assert_eq!(s(b"\x1b]8;;http://x\x1b\\link\x1b]8;;\x1b\\"), "link");
        assert_eq!(s(b"\x1bPq#0;2;0;0;0\x1b\\x"), "x");
        assert_eq!(s(b"\x1b(Bab\x1b=c\x1b>d"), "abcd");
        assert_eq!(s(b"tab\there\x07\x08!"), "tab\there!");
        // lone CR dropped, CR CR LF -> LF
        assert_eq!(s(b"50%\r100%\r\r\nok"), "50%100%\nok");
        // UTF-8 passes
        assert_eq!(s("日本語 \x1b[2Kテスト".as_bytes()), "日本語 テスト");
        // a cancelled CSI
        assert_eq!(s(b"\x1b[12\x18x"), "x");
    }

    #[test]
    fn stripping_survives_chunk_boundaries() {
        let input = b"\x1b[38;5;196mhello\x1b[0m\r\n\x1b]0;t\x07world\r\n";
        let whole = AnsiStripper::strip(input);
        for split in 1..input.len() {
            let mut st = AnsiStripper::default();
            let mut out = Vec::new();
            st.feed(&input[..split], &mut out);
            st.feed(&input[split..], &mut out);
            assert_eq!(out, whole, "split at {split}");
        }
        assert_eq!(whole, b"hello\nworld\n");
    }

    #[test]
    fn names() {
        assert!(valid_name("20261027-101500_ssh_r1.lab_22_ab12.log"));
        for bad in ["../x.log", "a/b.log", "x.txt", ".log", "a b.log", ""] {
            assert!(!valid_name(bad), "{bad}");
        }
        // 2026-10-27T10:15:00Z
        let n = file_name(1_793_096_100, "ssh", "2001:db8::1", 22, "ab12cd34");
        assert_eq!(n, "20261027-101500_ssh_2001_db8__1_22_ab12cd34.log");
        assert!(valid_name(&n));
        assert!(path_of(Path::new("/d"), "../etc.log").is_none());
    }

    #[tokio::test]
    async fn record_list_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("logs");
        assert!(list(&d, &[]).unwrap().is_empty());
        let mut r = Recorder::create(&d, "a.log".into(), true).await.unwrap();
        assert!(r.write(b"\x1b[1mbold\x1b[0m\r\n").await.unwrap());
        assert_eq!(r.name(), "a.log");
        r.finish().await;
        let mut raw = Recorder::create(&d, "b.log".into(), false).await.unwrap();
        raw.write(b"\x1b[1mbold").await.unwrap();
        raw.written = MAX_LOG_BYTES - 2;
        assert!(!raw.write(b"xyz").await.unwrap(), "limit reached");
        raw.finish().await;
        assert!(Recorder::create(&d, "a.log".into(), true).await.is_err());
        std::fs::write(d.join("ignored.txt"), "x").unwrap();
        let l = list(&d, &["b.log".to_owned()]).unwrap();
        assert_eq!(
            l.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["b.log", "a.log"]
        );
        assert!(l[0].active && !l[1].active);
        assert_eq!(std::fs::read(d.join("a.log")).unwrap(), b"bold\n");
        assert_eq!(std::fs::read(d.join("b.log")).unwrap(), b"\x1b[1mboldxy");
    }
}
