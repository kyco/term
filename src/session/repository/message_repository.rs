use super::MessageRepository;
use crate::repository::db::SqliteRepository;
use crate::session::entity::message_entity::MessageEntity;
use rusqlite::{params, Result, Row};

/// A user turn written to disk *before* the provider call, so a dropped
/// connection cannot swallow what the user typed. Promoted to `standard`
/// once the turn completes; excluded from conversation reads until then.
pub const PENDING: &str = "pending";

/// A row `sessions repair` identified as a re-insert of an earlier message.
/// Marked rather than deleted: the text stays in the database, out of the
/// conversation, and `--undo` puts it back.
pub const DUPLICATE: &str = "duplicate";

const MESSAGE_COLUMNS: &str = "id, session_id, role, content, message_type, compaction_metadata";

impl MessageRepository for SqliteRepository {
    type Error = rusqlite::Error;

    fn fetch_messages_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<MessageEntity>, Self::Error> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM messages
             WHERE session_id = ?1 AND message_type NOT IN (?2, ?3)
             ORDER BY sequence, rowid",
            MESSAGE_COLUMNS
        ))?;
        let rows = stmt.query_map(
            params![session_id, PENDING, DUPLICATE],
            row_to_message_entity(),
        )?;

        let mut messages = Vec::new();
        for message in rows {
            messages.push(message?);
        }
        Ok(messages)
    }

    fn add_message_to_session(&self, message: &MessageEntity) -> Result<(), Self::Error> {
        self.conn.execute(
            &format!(
                "INSERT INTO messages ({}, sequence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6,
                     (SELECT COALESCE(MAX(sequence), 0) + 1 FROM messages WHERE session_id = ?2))",
                MESSAGE_COLUMNS
            ),
            params![
                message.id,
                message.session_id,
                message.role,
                message.content,
                message.message_type,
                message.compaction_metadata
            ],
        )?;
        Ok(())
    }

    fn count_messages_for_session(&self, session_id: &str) -> Result<i64, Self::Error> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM messages
             WHERE session_id = ?1 AND message_type NOT IN (?2, ?3)",
            params![session_id, PENDING, DUPLICATE],
            |row| row.get(0),
        )
    }

    fn fetch_first_user_message(&self, session_id: &str) -> Result<Option<String>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT content FROM messages
             WHERE session_id = ?1 AND role = 'user' AND message_type NOT IN (?2, ?3)
             ORDER BY sequence, rowid LIMIT 1",
        )?;
        let mut rows = stmt.query(params![session_id, PENDING, DUPLICATE])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    fn promote_message(&self, message_id: &str) -> Result<(), Self::Error> {
        self.conn.execute(
            "UPDATE messages SET message_type = 'standard'
             WHERE id = ?1 AND message_type = ?2",
            params![message_id, PENDING],
        )?;
        Ok(())
    }

    fn fetch_pending_messages(&self, session_id: &str) -> Result<Vec<String>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT content FROM messages
             WHERE session_id = ?1 AND message_type = ?2
             ORDER BY sequence, rowid",
        )?;
        let rows = stmt.query_map(params![session_id, PENDING], |row| row.get::<_, String>(0))?;

        let mut contents = Vec::new();
        for content in rows {
            contents.push(content?);
        }
        Ok(contents)
    }

    fn discard_pending_messages(&self, session_id: &str) -> Result<(), Self::Error> {
        self.conn.execute(
            "DELETE FROM messages WHERE session_id = ?1 AND message_type = ?2",
            params![session_id, PENDING],
        )?;
        Ok(())
    }

    fn fetch_conversation_rows(&self, session_id: &str) -> Result<Vec<StoredRow>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content, message_type FROM messages
             WHERE session_id = ?1 AND message_type != ?2
             ORDER BY sequence, rowid",
        )?;
        let rows = stmt.query_map(params![session_id, PENDING], |row| {
            Ok(StoredRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                message_type: row.get(3)?,
            })
        })?;

        let mut stored = Vec::new();
        for row in rows {
            stored.push(row?);
        }
        Ok(stored)
    }

    fn mark_duplicates(&self, ids: &[String]) -> Result<(), Self::Error> {
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE messages SET message_type = ?1 WHERE id = ?2",
                params![DUPLICATE, id],
            )?;
        }
        tx.commit()
    }

    fn restore_duplicates(&self) -> Result<usize, Self::Error> {
        self.conn.execute(
            "UPDATE messages SET message_type = 'standard' WHERE message_type = ?1",
            params![DUPLICATE],
        )
    }

    fn search_messages(&self, query: &str, limit: usize) -> Result<Vec<MessageMatch>, Self::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.name, m.role, m.content
             FROM messages m
             JOIN sessions s ON s.id = m.session_id
             WHERE m.content LIKE ?1 ESCAPE '\\' AND m.message_type NOT IN (?2, ?3)
             ORDER BY s.last_used_at DESC, m.sequence
             LIMIT ?4",
        )?;
        let pattern = format!("%{}%", escape_like(query));
        let rows = stmt.query_map(params![pattern, PENDING, DUPLICATE, limit as i64], |row| {
            Ok(MessageMatch {
                session_name: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
            })
        })?;

        let mut matches = Vec::new();
        for row in rows {
            matches.push(row?);
        }
        Ok(matches)
    }
}

/// A stored message row as `sessions repair` needs to see it, duplicates
/// included.
pub struct StoredRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub message_type: String,
}

/// One hit from a full-conversation content search.
pub struct MessageMatch {
    pub session_name: String,
    pub role: String,
    pub content: String,
}

/// LIKE treats `%` and `_` as wildcards; a user searching for "100%" means
/// the literal characters.
fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn row_to_message_entity() -> fn(&Row) -> Result<MessageEntity> {
    |row| {
        let id: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let role: String = row.get(2)?;
        let content: String = row.get(3)?;
        let message_type: String = row.get(4).unwrap_or_else(|_| "standard".to_string());
        let compaction_metadata: Option<String> = row.get(5).ok();

        Ok(MessageEntity::new_with_type(
            id,
            session_id,
            role,
            content,
            message_type,
            compaction_metadata,
        ))
    }
}
