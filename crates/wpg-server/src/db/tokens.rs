//! API tokens (`api_tokens` table): see [`crate::auth::token`].

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{finish_user, user_from_row, Db, DbError, User};

/// An API token as listed (never the secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TokenInfo {
    /// Token id (for revoking).
    pub id: i64,
    /// Name given at creation.
    pub name: String,
    /// The first characters of the secret (`wpg_` + 8 hex digits), to recognise it.
    pub prefix: String,
    /// Owner: requests with the token act as this user, with its role.
    pub user: String,
    /// Creation time (Unix seconds).
    pub created: i64,
    /// Last use (Unix seconds, updated at most once a minute).
    #[serde(default)]
    pub last_used: Option<i64>,
    /// Expiry (Unix seconds); `null`: never expires.
    #[serde(default)]
    pub expires: Option<i64>,
}

/// A new token row.
#[derive(Debug, Clone)]
pub struct NewToken<'a> {
    /// Owner.
    pub user_id: i64,
    /// Name.
    pub name: &'a str,
    /// SHA-256 of the secret (hex).
    pub hash: &'a str,
    /// Displayed prefix.
    pub prefix: &'a str,
    /// Now.
    pub now: i64,
    /// Expiry.
    pub expires: Option<i64>,
}

/// A live token with its user (the result of a bearer lookup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAuth {
    /// Token id.
    pub id: i64,
    /// Owner.
    pub user: User,
    /// Last use.
    pub last_used: Option<i64>,
}

const LIST: &str = "SELECT t.id, t.name, t.prefix, u.name, t.created, t.last_used, t.expires
     FROM api_tokens t JOIN users u ON u.id = t.user_id";

fn info_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TokenInfo> {
    Ok(TokenInfo {
        id: r.get(0)?,
        name: r.get(1)?,
        prefix: r.get(2)?,
        user: r.get(3)?,
        created: r.get(4)?,
        last_used: r.get(5)?,
        expires: r.get(6)?,
    })
}

impl Db {
    /// Stores a new token.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn insert_token(&self, t: &NewToken<'_>) -> Result<TokenInfo, DbError> {
        self.with(|c| {
            c.execute(
                "INSERT INTO api_tokens (user_id, name, hash, prefix, created, expires)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![t.user_id, t.name, t.hash, t.prefix, t.now, t.expires],
            )?;
            let id = c.last_insert_rowid();
            Ok(c.query_row(&format!("{LIST} WHERE t.id = ?1"), [id], info_from_row)?)
        })
    }

    /// Tokens, oldest first; only those of `user_id` when given.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn tokens(&self, user_id: Option<i64>) -> Result<Vec<TokenInfo>, DbError> {
        self.with(|c| {
            let mut st = c.prepare(&format!(
                "{LIST} WHERE ?1 IS NULL OR t.user_id = ?1 ORDER BY t.created, t.id"
            ))?;
            let rows = st.query_map([user_id], info_from_row)?;
            Ok(rows.collect::<Result<_, _>>()?)
        })
    }

    /// Deletes a token (of `user_id` only, when given); `false` when there is none.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn delete_token(&self, id: i64, user_id: Option<i64>) -> Result<bool, DbError> {
        self.with(|c| {
            Ok(c.execute(
                "DELETE FROM api_tokens WHERE id = ?1 AND (?2 IS NULL OR user_id = ?2)",
                params![id, user_id],
            )? == 1)
        })
    }

    /// The live (not expired at `now`) token with SHA-256 `hash`.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn token_by_hash(&self, hash: &str, now: i64) -> Result<Option<TokenAuth>, DbError> {
        self.with(|c| {
            c.query_row(
                "SELECT t.id, t.last_used, u.id, u.name, u.role, u.password_hash, u.created
                 FROM api_tokens t JOIN users u ON u.id = t.user_id
                 WHERE t.hash = ?1 AND (t.expires IS NULL OR t.expires > ?2)",
                params![hash, now],
                |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, user_from_row(r, 2)?)),
            )
            .optional()?
            .map(|(id, last_used, user)| {
                Ok(TokenAuth {
                    id,
                    user: finish_user(user)?,
                    last_used,
                })
            })
            .transpose()
        })
    }

    /// Records the use of a token.
    ///
    /// # Errors
    /// `SQLite` errors.
    pub fn touch_token(&self, id: i64, now: i64) -> Result<(), DbError> {
        self.with(|c| {
            c.execute(
                "UPDATE api_tokens SET last_used = ?2 WHERE id = ?1",
                params![id, now],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Role;

    #[test]
    fn token_rows() {
        let db = Db::open_in_memory().unwrap();
        let u = db
            .create_user("admin", Role::Admin, "h", 1)
            .unwrap()
            .unwrap();
        let v = db
            .create_user("ops", Role::Viewer, "h", 1)
            .unwrap()
            .unwrap();
        let new = |user_id, name, hash, expires| NewToken {
            user_id,
            name,
            hash,
            prefix: "wpg_00000000",
            now: 100,
            expires,
        };
        let a = db.insert_token(&new(u.id, "ci", "h-a", None)).unwrap();
        assert_eq!(a.user, "admin");
        assert_eq!(a.last_used, None);
        db.insert_token(&new(v.id, "ro", "h-b", Some(200))).unwrap();
        assert_eq!(db.tokens(None).unwrap().len(), 2);
        assert_eq!(db.tokens(Some(v.id)).unwrap()[0].name, "ro");

        let t = db.token_by_hash("h-a", 150).unwrap().unwrap();
        assert_eq!((t.id, t.user.role), (a.id, Role::Admin));
        assert_eq!(
            db.token_by_hash("h-b", 150).unwrap().unwrap().user.role,
            Role::Viewer
        );
        assert!(db.token_by_hash("h-b", 200).unwrap().is_none(), "expired");
        assert!(db.token_by_hash("nope", 150).unwrap().is_none());

        db.touch_token(a.id, 160).unwrap();
        assert_eq!(db.tokens(Some(u.id)).unwrap()[0].last_used, Some(160));

        assert!(!db.delete_token(a.id, Some(v.id)).unwrap(), "not the owner");
        assert!(db.delete_token(a.id, None).unwrap());
        assert!(db.token_by_hash("h-a", 150).unwrap().is_none());
        assert!(!db.delete_token(a.id, None).unwrap());
    }
}
