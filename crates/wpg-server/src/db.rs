//! The `SQLite` database `<data_dir>/wpg.sqlite` (TODO §3 "DB ハイブリッド":
//! templates stay YAML files; users, sessions, login attempts and the job history
//! live here).
//!
//! One connection behind a mutex: every statement is a short indexed lookup /
//! insert, so the handlers call it directly instead of going through a pool or
//! `spawn_blocking` (WAL mode keeps writers from blocking for long). The schema is
//! versioned by the `schema_version` table; [`MIGRATIONS`] are applied in order on
//! open.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use crate::jobs::JobInfo;

/// Finished jobs kept in the database (the in-memory ring keeps fewer).
pub const HISTORY_KEEP: usize = 1000;

/// Login attempts older than this are purged (seconds).
const ATTEMPTS_KEEP_SECS: i64 = 30 * 24 * 60 * 60;

/// Schema migrations; index + 1 is the schema version after applying it.
const MIGRATIONS: &[&str] = &[
    // 1: users, sessions, login attempts, job history
    "CREATE TABLE users (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         name TEXT NOT NULL UNIQUE,
         role TEXT NOT NULL CHECK (role IN ('admin', 'operator', 'viewer')),
         password_hash TEXT NOT NULL,
         created INTEGER NOT NULL
     );
     CREATE TABLE sessions (
         id TEXT PRIMARY KEY,            -- SHA-256 of the cookie token (hex)
         user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
         csrf TEXT NOT NULL,
         created INTEGER NOT NULL,
         expires INTEGER NOT NULL,
         last_seen INTEGER NOT NULL,
         ip TEXT,
         ua TEXT
     );
     CREATE INDEX sessions_expires ON sessions(expires);
     CREATE TABLE login_attempts (
         ip TEXT NOT NULL,
         ts INTEGER NOT NULL,
         ok INTEGER NOT NULL,
         username TEXT
     );
     CREATE INDEX login_attempts_ts ON login_attempts(ts);
     CREATE TABLE job_history (
         id TEXT PRIMARY KEY,
         name TEXT NOT NULL,
         state TEXT NOT NULL,
         created INTEGER NOT NULL,
         finished INTEGER,
         record TEXT NOT NULL            -- JobInfo as JSON
     );
     CREATE INDEX job_history_finished ON job_history(finished);",
];

/// Database errors.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DbError {
    /// `SQLite` error.
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A stored record does not decode.
    #[error("database record: {0}")]
    Decode(String),
    /// The database was written by a newer wpg.
    #[error("database schema version {found} is newer than this build supports ({supported})")]
    TooNew {
        /// Version in the file.
        found: usize,
        /// Latest version known to this build.
        supported: usize,
    },
}

/// A user role (only `admin` is created for now; operator / viewer are P1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Everything.
    Admin,
    /// Send packets (P1).
    Operator,
    /// Read-only (P1).
    Viewer,
}

impl Role {
    /// The stored name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Operator => "operator",
            Role::Viewer => "viewer",
        }
    }

    fn parse(s: &str) -> Result<Self, DbError> {
        match s {
            "admin" => Ok(Role::Admin),
            "operator" => Ok(Role::Operator),
            "viewer" => Ok(Role::Viewer),
            _ => Err(DbError::Decode(format!("unknown role {s}"))),
        }
    }
}

/// A user row.
#[derive(Clone, PartialEq, Eq)]
pub struct User {
    /// Row id.
    pub id: i64,
    /// Login name.
    pub name: String,
    /// Role.
    pub role: Role,
    /// argon2 PHC string.
    pub password_hash: String,
    /// Creation time (Unix seconds).
    pub created: i64,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// A session row (joined with its user).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    /// SHA-256 of the cookie token.
    pub id: String,
    /// Owner.
    pub user: User,
    /// CSRF token bound to the session.
    pub csrf: String,
    /// Creation time.
    pub created: i64,
    /// Expiry time.
    pub expires: i64,
    /// Last request.
    pub last_seen: i64,
}

/// A new session.
#[derive(Debug, Clone)]
pub struct NewSession<'a> {
    /// SHA-256 of the cookie token.
    pub id: &'a str,
    /// Owner.
    pub user_id: i64,
    /// CSRF token.
    pub csrf: &'a str,
    /// Now.
    pub now: i64,
    /// Expiry.
    pub expires: i64,
    /// Client address.
    pub ip: Option<&'a str>,
    /// `User-Agent`.
    pub ua: Option<&'a str>,
}

/// The database handle (cheap to clone).
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").finish_non_exhaustive()
    }
}

fn user_from_row(r: &rusqlite::Row<'_>, base: usize) -> rusqlite::Result<(User, String)> {
    Ok((
        User {
            id: r.get(base)?,
            name: r.get(base + 1)?,
            role: Role::Admin, // replaced below (needs a fallible conversion)
            password_hash: r.get(base + 3)?,
            created: r.get(base + 4)?,
        },
        r.get(base + 2)?,
    ))
}

fn finish_user((mut u, role): (User, String)) -> Result<User, DbError> {
    u.role = Role::parse(&role)?;
    Ok(u)
}

impl Db {
    /// Opens (creating) the database file and migrates it.
    ///
    /// # Errors
    /// `SQLite` errors, a schema newer than this build.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::setup(conn)
    }

    /// An in-memory database (tests).
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<Self, DbError> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError>) -> Result<T, DbError> {
        let c = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        f(&c)
    }

    /// Current schema version.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn schema_version(&self) -> Result<usize, DbError> {
        self.with(|c| Ok(version(c)?))
    }

    // ------------------------------------------------------------ users

    /// Number of users.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn user_count(&self) -> Result<u64, DbError> {
        self.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?))
    }

    /// A user by name.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn user_by_name(&self, name: &str) -> Result<Option<User>, DbError> {
        self.with(|c| {
            c.query_row(
                "SELECT id, name, role, password_hash, created FROM users WHERE name = ?1",
                [name],
                |r| user_from_row(r, 0),
            )
            .optional()?
            .map(finish_user)
            .transpose()
        })
    }

    /// Creates a user; `None` when the name is taken.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn create_user(
        &self,
        name: &str,
        role: Role,
        password_hash: &str,
        now: i64,
    ) -> Result<Option<User>, DbError> {
        self.with(|c| {
            let n = c.execute(
                "INSERT OR IGNORE INTO users (name, role, password_hash, created)
                 VALUES (?1, ?2, ?3, ?4)",
                params![name, role.as_str(), password_hash, now],
            )?;
            Ok((n == 1).then(|| User {
                id: c.last_insert_rowid(),
                name: name.to_owned(),
                role,
                password_hash: password_hash.to_owned(),
                created: now,
            }))
        })
    }

    /// Creates the first user atomically: `None` when any user already exists
    /// (the setup screen can only succeed once).
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn create_first_user(
        &self,
        name: &str,
        role: Role,
        password_hash: &str,
        now: i64,
    ) -> Result<Option<User>, DbError> {
        self.with(|c| {
            let n = c.execute(
                "INSERT INTO users (name, role, password_hash, created)
                 SELECT ?1, ?2, ?3, ?4 WHERE NOT EXISTS (SELECT 1 FROM users)",
                params![name, role.as_str(), password_hash, now],
            )?;
            Ok((n == 1).then(|| User {
                id: c.last_insert_rowid(),
                name: name.to_owned(),
                role,
                password_hash: password_hash.to_owned(),
                created: now,
            }))
        })
    }

    /// Replaces a user's password hash and ends their sessions.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn set_password(&self, user_id: i64, password_hash: &str) -> Result<(), DbError> {
        self.with(|c| {
            c.execute(
                "UPDATE users SET password_hash = ?2 WHERE id = ?1",
                params![user_id, password_hash],
            )?;
            c.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])?;
            Ok(())
        })
    }

    // ------------------------------------------------------------ sessions

    /// Stores a new session.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn insert_session(&self, s: &NewSession<'_>) -> Result<(), DbError> {
        self.with(|c| {
            c.execute(
                "INSERT INTO sessions (id, user_id, csrf, created, expires, last_seen, ip, ua)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?6, ?7)",
                params![s.id, s.user_id, s.csrf, s.now, s.expires, s.ip, s.ua],
            )?;
            Ok(())
        })
    }

    /// A live (not expired at `now`) session with its user.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn session(&self, id: &str, now: i64) -> Result<Option<SessionRow>, DbError> {
        self.with(|c| {
            let row = c
                .query_row(
                    "SELECT s.id, s.csrf, s.created, s.expires, s.last_seen,
                            u.id, u.name, u.role, u.password_hash, u.created
                     FROM sessions s JOIN users u ON u.id = s.user_id
                     WHERE s.id = ?1 AND s.expires > ?2",
                    params![id, now],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            user_from_row(r, 5)?,
                        ))
                    },
                )
                .optional()?;
            row.map(|(id, csrf, created, expires, last_seen, user)| {
                Ok(SessionRow {
                    id,
                    user: finish_user(user)?,
                    csrf,
                    created,
                    expires,
                    last_seen,
                })
            })
            .transpose()
        })
    }

    /// Sliding expiry: records activity and moves the expiry.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn touch_session(&self, id: &str, now: i64, expires: i64) -> Result<(), DbError> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET last_seen = ?2, expires = ?3 WHERE id = ?1",
                params![id, now, expires],
            )?;
            Ok(())
        })
    }

    /// Deletes a session (logout).
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn delete_session(&self, id: &str) -> Result<bool, DbError> {
        self.with(|c| Ok(c.execute("DELETE FROM sessions WHERE id = ?1", [id])? == 1))
    }

    /// Removes expired sessions and old login attempts; returns the number of
    /// sessions removed.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn purge(&self, now: i64) -> Result<usize, DbError> {
        self.with(|c| {
            let n = c.execute("DELETE FROM sessions WHERE expires <= ?1", [now])?;
            c.execute(
                "DELETE FROM login_attempts WHERE ts < ?1",
                [now - ATTEMPTS_KEEP_SECS],
            )?;
            Ok(n)
        })
    }

    // ------------------------------------------------------------ login attempts

    /// Records a login attempt (audit trail).
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn record_attempt(
        &self,
        ip: &str,
        username: Option<&str>,
        now: i64,
        ok: bool,
    ) -> Result<(), DbError> {
        self.with(|c| {
            c.execute(
                "INSERT INTO login_attempts (ip, ts, ok, username) VALUES (?1, ?2, ?3, ?4)",
                params![ip, now, ok, username],
            )?;
            Ok(())
        })
    }

    /// Failed attempts from `ip` since `since`.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn failed_attempts_since(&self, ip: &str, since: i64) -> Result<u64, DbError> {
        self.with(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM login_attempts WHERE ip = ?1 AND ts >= ?2 AND ok = 0",
                params![ip, since],
                |r| r.get(0),
            )?)
        })
    }

    // ------------------------------------------------------------ job history

    /// Inserts or replaces a finished job and trims the table to
    /// [`HISTORY_KEEP`] rows.
    ///
    /// # Errors
    /// `SQLite` / JSON errors.
    pub fn save_job(&self, info: &JobInfo) -> Result<(), DbError> {
        let record = serde_json::to_string(info).map_err(|e| DbError::Decode(e.to_string()))?;
        self.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO job_history (id, name, state, created, finished, record)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    info.id,
                    info.name,
                    info.status.name(),
                    i64::try_from(info.created).unwrap_or(i64::MAX),
                    info.finished.map(|f| i64::try_from(f).unwrap_or(i64::MAX)),
                    record
                ],
            )?;
            c.execute(
                "DELETE FROM job_history WHERE id NOT IN
                 (SELECT id FROM job_history ORDER BY finished DESC, created DESC LIMIT ?1)",
                [i64::try_from(HISTORY_KEEP).unwrap_or(i64::MAX)],
            )?;
            Ok(())
        })
    }

    /// The newest `limit` finished jobs, newest first (undecodable rows are
    /// skipped).
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn jobs(&self, limit: usize) -> Result<Vec<JobInfo>, DbError> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT record FROM job_history ORDER BY finished DESC, created DESC LIMIT ?1",
            )?;
            let rows = st.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |r| {
                r.get::<_, String>(0)
            })?;
            let mut out = Vec::new();
            for r in rows {
                if let Ok(info) = serde_json::from_str::<JobInfo>(&r?) {
                    out.push(info);
                }
            }
            Ok(out)
        })
    }

    /// Imports the JSON job records of wpg ≤ M3 (`<data_dir>/history/*.json`)
    /// once, then renames the directory to `history.imported`. Returns the number
    /// of imported jobs.
    ///
    /// # Errors
    /// `SQLite` errors (unreadable files are skipped).
    pub fn import_legacy_history(&self, dir: &Path) -> Result<usize, DbError> {
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut n = 0;
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let Some(info) = std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<JobInfo>(&b).ok())
            else {
                continue;
            };
            self.save_job(&info)?;
            n += 1;
        }
        let target = dir.with_file_name("history.imported");
        if let Err(e) = std::fs::rename(dir, &target) {
            tracing::warn!("cannot rename {}: {e}", dir.display());
        }
        Ok(n)
    }
}

fn version(c: &Connection) -> rusqlite::Result<usize> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")?;
    let v: Option<i64> = c
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .optional()?
        .flatten();
    Ok(usize::try_from(v.unwrap_or(0)).unwrap_or(0))
}

fn migrate(c: &mut Connection) -> Result<(), DbError> {
    let current = version(c)?;
    if current > MIGRATIONS.len() {
        return Err(DbError::TooNew {
            found: current,
            supported: MIGRATIONS.len(),
        });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = c.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute("DELETE FROM schema_version", [])?;
        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [i64::try_from(i + 1).unwrap_or(i64::MAX)],
        )?;
        tx.commit()?;
        tracing::debug!("database migrated to schema version {}", i + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{EndReason, JobSpec, JobStatus};

    fn job(id: &str, finished: u64) -> JobInfo {
        JobInfo {
            id: id.into(),
            name: format!("job {id}"),
            spec: JobSpec::default(),
            status: JobStatus::Finished(EndReason::CountReached),
            backend: "null".into(),
            iface: String::new(),
            rate: "unlimited".into(),
            workers: 1,
            plans: vec![],
            created: finished - 10,
            started: Some(finished - 5),
            finished: Some(finished),
            stats: None,
            report: None,
        }
    }

    #[test]
    fn migrations_are_idempotent_and_versioned() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wpg.sqlite");
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), MIGRATIONS.len());
        db.create_user("admin", Role::Admin, "$argon2id$x", 1)
            .unwrap()
            .unwrap();
        drop(db);
        let db = Db::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), MIGRATIONS.len());
        assert_eq!(db.user_count().unwrap(), 1);
        // a newer schema is refused
        db.with(|c| {
            c.execute("UPDATE schema_version SET version = 999", [])?;
            Ok(())
        })
        .unwrap();
        drop(db);
        assert!(matches!(Db::open(&path), Err(DbError::TooNew { .. })));
    }

    #[test]
    fn users_and_sessions() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.user_count().unwrap(), 0);
        let u = db
            .create_first_user("admin", Role::Admin, "h1", 100)
            .unwrap()
            .unwrap();
        assert!(db
            .create_first_user("other", Role::Admin, "h2", 100)
            .unwrap()
            .is_none());
        assert!(db
            .create_user("admin", Role::Viewer, "h3", 1)
            .unwrap()
            .is_none());
        assert_eq!(db.user_by_name("admin").unwrap().unwrap(), u);
        assert!(db.user_by_name("nobody").unwrap().is_none());

        db.insert_session(&NewSession {
            id: "s1",
            user_id: u.id,
            csrf: "c1",
            now: 100,
            expires: 200,
            ip: Some("127.0.0.1"),
            ua: None,
        })
        .unwrap();
        let s = db.session("s1", 150).unwrap().unwrap();
        assert_eq!(s.user.name, "admin");
        assert_eq!(s.csrf, "c1");
        assert!(db.session("s1", 200).unwrap().is_none(), "expired");
        db.touch_session("s1", 190, 300).unwrap();
        assert_eq!(db.session("s1", 250).unwrap().unwrap().last_seen, 190);
        assert!(db.session("nope", 150).unwrap().is_none());

        // a password change ends the sessions
        db.set_password(u.id, "h9").unwrap();
        assert!(db.session("s1", 150).unwrap().is_none());
        assert_eq!(
            db.user_by_name("admin").unwrap().unwrap().password_hash,
            "h9"
        );

        db.insert_session(&NewSession {
            id: "s2",
            user_id: u.id,
            csrf: "c2",
            now: 100,
            expires: 200,
            ip: None,
            ua: None,
        })
        .unwrap();
        assert_eq!(db.purge(300).unwrap(), 1);
        assert!(!db.delete_session("s2").unwrap());
    }

    #[test]
    fn login_attempts() {
        let db = Db::open_in_memory().unwrap();
        db.record_attempt("1.2.3.4", Some("admin"), 10, false)
            .unwrap();
        db.record_attempt("1.2.3.4", None, 20, false).unwrap();
        db.record_attempt("1.2.3.4", None, 30, true).unwrap();
        db.record_attempt("5.6.7.8", None, 30, false).unwrap();
        assert_eq!(db.failed_attempts_since("1.2.3.4", 0).unwrap(), 2);
        assert_eq!(db.failed_attempts_since("1.2.3.4", 15).unwrap(), 1);
    }

    #[test]
    fn job_history_round_trip_and_legacy_import() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("history");
        std::fs::create_dir_all(&legacy).unwrap();
        for (id, t) in [("a", 1000), ("b", 3000)] {
            std::fs::write(
                legacy.join(format!("{id}.json")),
                serde_json::to_vec(&job(id, t)).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(legacy.join("junk.json"), b"{").unwrap();
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.import_legacy_history(&legacy).unwrap(), 2);
        assert!(!legacy.exists());
        assert!(dir.path().join("history.imported").is_dir());
        assert_eq!(db.import_legacy_history(&legacy).unwrap(), 0);

        db.save_job(&job("c", 2000)).unwrap();
        let ids: Vec<String> = db.jobs(10).unwrap().into_iter().map(|j| j.id).collect();
        assert_eq!(ids, ["b", "c", "a"]);
        assert_eq!(db.jobs(1).unwrap()[0], job("b", 3000));
        // replace
        let mut b = job("b", 3000);
        b.name = "renamed".into();
        db.save_job(&b).unwrap();
        assert_eq!(db.jobs(1).unwrap()[0].name, "renamed");
        assert_eq!(db.jobs(10).unwrap().len(), 3);
    }
}
