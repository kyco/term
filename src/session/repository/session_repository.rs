use super::SessionRepository;
use crate::{repository::db::SqliteRepository, session::entity::session_entity::SessionEntity};
use chrono::{Duration, NaiveDateTime};
use rusqlite::{params, Result, Row};

const DATE_TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Columns every session read shares. `last_used_at` is backfilled by the
/// migration, but a row written by an older binary can still be NULL, so it
/// falls back to the legacy `expires_at - 24h` encoding.
const SESSION_COLUMNS: &str =
    "id, name, expires_at, current, COALESCE(last_used_at, datetime(expires_at, '-24 hours'))";

impl SessionRepository for SqliteRepository {
    type Error = rusqlite::Error;

    fn fetch_all_sessions(&self) -> Result<Vec<SessionEntity>, Self::Error> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {} FROM sessions", SESSION_COLUMNS))?;
        let rows = stmt.query_map([], row_to_session_entity())?;

        let mut sessions = Vec::new();
        for session in rows {
            sessions.push(session?);
        }
        Ok(sessions)
    }

    fn fetch_session_by_name(&self, name: &str) -> Result<SessionEntity, Self::Error> {
        let session = self.conn.query_row(
            &format!("SELECT {} FROM sessions WHERE name = ?1", SESSION_COLUMNS),
            params![name],
            row_to_session_entity(),
        )?;

        Ok(session)
    }

    fn add_session(
        &self,
        id: &str,
        name: &str,
        last_used_at: NaiveDateTime,
        current: bool,
    ) -> Result<(), Self::Error> {
        self.conn.execute(
            "INSERT INTO sessions (id, name, expires_at, current, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                name,
                legacy_expires_at(last_used_at),
                i32::from(current),
                format_time(last_used_at),
            ],
        )?;
        Ok(())
    }

    /// Write the session row whether or not it already exists.
    ///
    /// `update_session` alone silently affected zero rows for any session that
    /// had never been inserted, which is how conversations ended up as
    /// messages pointing at a session that did not exist.
    fn upsert_session(
        &self,
        id: &str,
        name: &str,
        last_used_at: NaiveDateTime,
        current: bool,
    ) -> Result<(), Self::Error> {
        self.conn.execute(
            "INSERT INTO sessions (id, name, expires_at, current, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 expires_at = excluded.expires_at,
                 current = excluded.current,
                 last_used_at = excluded.last_used_at",
            params![
                id,
                name,
                legacy_expires_at(last_used_at),
                i32::from(current),
                format_time(last_used_at),
            ],
        )?;
        Ok(())
    }

    fn remove_current_from_all(&self) -> Result<(), Self::Error> {
        self.conn
            .execute("UPDATE sessions SET current = 0", params![])?;
        Ok(())
    }

    fn delete_session(&self, session_id: &str) -> Result<(), Self::Error> {
        // Start a transaction to ensure both session and messages are deleted atomically
        let tx = self.conn.unchecked_transaction()?;

        // Delete all messages for this session first (foreign key constraint)
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;

        // Delete the session itself
        let rows_affected =
            tx.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;

        // Commit the transaction
        tx.commit()?;

        // Check if session was actually deleted
        if rows_affected == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        Ok(())
    }
}

fn format_time(value: NaiveDateTime) -> String {
    value.format(DATE_TIME_FORMAT).to_string()
}

/// `expires_at` is retained purely so a session written by this binary stays
/// readable by an older one, which sorts on it. It carries no other meaning.
fn legacy_expires_at(last_used_at: NaiveDateTime) -> String {
    format_time(last_used_at + Duration::hours(24))
}

fn parse_time(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, DATE_TIME_FORMAT)
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .unwrap_or_default()
}

fn row_to_session_entity() -> fn(&Row) -> Result<SessionEntity> {
    |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let expires_at_str: String = row.get(2)?;
        let current: i32 = row.get(3)?;
        let last_used_at_str: String = row.get(4)?;

        Ok(SessionEntity::new(
            id,
            name,
            parse_time(&expires_at_str),
            current,
            parse_time(&last_used_at_str),
        ))
    }
}
