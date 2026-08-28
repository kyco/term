use rusqlite::{params, Connection, Result, TransactionBehavior};

pub struct SqliteRepository {
    pub(crate) conn: Connection,
}

impl SqliteRepository {
    pub fn new(path: &str) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        // A second termai holding the write lock used to abort the running
        // chat outright; wait for it instead. WAL lets a read (a listing, a
        // shell completion) run while a chat is writing.
        conn.busy_timeout(std::time::Duration::from_secs(10))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.pragma_update(None, "synchronous", "FULL")?;

        // One process migrates at a time. Two instances starting together
        // used to both see a column as missing and both ALTER, and the loser
        // died with "duplicate column name" before the chat even opened.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        migrate(&tx)?;
        tx.commit()?;

        Ok(Self { conn })
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    create_table_messages(conn)?;
    create_table_config(conn)?;
    create_table_sessions(conn)?;
    create_table_conversation_branches(conn)?;
    create_table_branch_messages(conn)?;
    create_table_branch_metadata(conn)?;
    migrate_messages_id_column(conn)?;
    messages_add_session_id_column(conn)?;
    messages_add_role_column(conn)?;
    messages_add_type_columns(conn)?;
    sessions_add_current_column(conn)?;
    sessions_rename_column_key_to_name(conn)?;
    sessions_add_last_used_column(conn)?;
    sessions_enforce_unique_names(conn)?;
    messages_add_sequence_column(conn)?;
    create_message_indexes(conn)?;
    Ok(())
}

/// `ALTER TABLE ADD COLUMN`, tolerating a column another process added first.
fn add_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<bool> {
    if column_exists(conn, table, column)? {
        return Ok(false);
    }
    match conn.execute(
        &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition),
        [],
    ) {
        Ok(_) => Ok(true),
        Err(err) if is_duplicate_column(&err) => Ok(false),
        Err(err) => Err(err),
    }
}

fn is_duplicate_column(error: &rusqlite::Error) -> bool {
    error.to_string().contains("duplicate column name")
}

fn create_table_messages(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
                id TEXT NOT NULL PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL
            )",
        [],
    )?;
    Ok(())
}

/// Conversation order used to be implicit in SQLite's rowid, which only held
/// while every read was a full table scan. `sequence` makes it explicit so
/// adding an index on `session_id` can never scramble a conversation.
fn messages_add_sequence_column(conn: &Connection) -> Result<()> {
    if add_column(conn, "messages", "sequence", "INTEGER NOT NULL DEFAULT 0")? {
        // Backfill from rowid: for existing rows insertion order *is* rowid order.
        conn.execute("UPDATE messages SET sequence = rowid", [])?;
    }
    Ok(())
}

fn create_message_indexes(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session ON messages (session_id, sequence)",
        [],
    )?;
    Ok(())
}

/// `expires_at` was overloaded as a last-used clock (it is stamped at
/// now + 24h on every touch) while also being displayed as an expiry the
/// user could not act on. `last_used_at` records the honest value.
fn sessions_add_last_used_column(conn: &Connection) -> Result<()> {
    if add_column(conn, "sessions", "last_used_at", "TEXT")? {
        // Historic rows: expires_at was always stamped at last-use + 24h.
        conn.execute(
            "UPDATE sessions
             SET last_used_at = datetime(expires_at, '-24 hours')
             WHERE last_used_at IS NULL",
            [],
        )?;
    }
    Ok(())
}

/// Two sessions sharing a name make one of them permanently unreachable:
/// every lookup goes through `fetch_session_by_name`, which returns the first
/// row. Suffix any pre-existing collisions, then make it impossible.
fn sessions_enforce_unique_names(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id FROM sessions WHERE rowid NOT IN
             (SELECT MIN(rowid) FROM sessions GROUP BY name)",
    )?;
    let shadowed: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;
    drop(stmt);

    for (index, id) in shadowed.iter().enumerate() {
        conn.execute(
            "UPDATE sessions SET name = name || ?1 WHERE id = ?2",
            params![format!("-{}", index + 2), id],
        )?;
    }

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_name ON sessions (name)",
        [],
    )?;
    Ok(())
}

/// True when `table` has a column called `column`.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut found = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            found = true;
            break;
        }
    }
    Ok(found)
}

fn create_table_config(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS config (
                id INTEGER PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL
            )",
        [],
    )?;
    Ok(())
}

fn create_table_sessions(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                current INTEGER NOT NULL DEFAULT 0
            )",
        [],
    )?;
    // conversation_branches declares a FK on sessions(id); without a unique
    // index on id, SQLite rejects every branch insert with "foreign key mismatch"
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_id ON sessions (id)",
        [],
    )?;
    Ok(())
}

fn migrate_messages_id_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
    let mut old_id_schema = false;
    let rows = stmt.query_map([], |row| {
        let col_name: String = row.get(1)?;
        let col_type: String = row.get(2)?;
        Ok((col_name, col_type))
    })?;
    for col in rows {
        let (name, col_type) = col?;
        if name == "id" && col_type.eq_ignore_ascii_case("INTEGER") {
            old_id_schema = true;
            break;
        }
    }
    drop(stmt);

    if old_id_schema {
        conn.execute("ALTER TABLE messages RENAME TO messages_old", [])?;
        create_table_messages(conn)?;
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content)
             SELECT CAST(id AS TEXT), session_id, role, content
             FROM messages_old",
            [],
        )?;
        conn.execute("DROP TABLE messages_old", [])?;
    }
    Ok(())
}

fn messages_add_session_id_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
    let mut has_session_id = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        if col? == "session_id" {
            has_session_id = true;
            break;
        }
    }
    if !has_session_id {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN session_id TEXT NOT NULL",
            [],
        )?;
    }
    drop(stmt);
    Ok(())
}

fn messages_add_role_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
    let mut has_role = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        if col? == "role" {
            has_role = true;
            break;
        }
    }
    if !has_role {
        conn.execute("ALTER TABLE messages ADD COLUMN role TEXT NOT NULL", [])?;
    }
    drop(stmt);
    Ok(())
}

fn messages_add_type_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
    let mut has_message_type = false;
    let mut has_compaction_metadata = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        let col_name = col?;
        if col_name == "message_type" {
            has_message_type = true;
        }
        if col_name == "compaction_metadata" {
            has_compaction_metadata = true;
        }
    }
    drop(stmt);

    if !has_message_type {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN message_type TEXT NOT NULL DEFAULT 'standard'",
            [],
        )?;
    }
    if !has_compaction_metadata {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN compaction_metadata TEXT",
            [],
        )?;
    }
    Ok(())
}

fn sessions_add_current_column(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
    let mut has_current = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        if col? == "current" {
            has_current = true;
            break;
        }
    }
    if !has_current {
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN current INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    drop(stmt);
    Ok(())
}

fn sessions_rename_column_key_to_name(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(sessions)")?;
    let mut has_key = false;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        if col? == "key" {
            has_key = true;
            break;
        }
    }
    drop(stmt);
    if has_key {
        conn.execute("ALTER TABLE sessions RENAME COLUMN key TO name", [])?;
    }
    Ok(())
}

fn create_table_conversation_branches(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS conversation_branches (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            parent_branch_id TEXT,
            branch_name TEXT,
            description TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_activity DATETIME DEFAULT CURRENT_TIMESTAMP,
            status TEXT DEFAULT 'active',
            FOREIGN KEY (session_id) REFERENCES sessions (id),
            FOREIGN KEY (parent_branch_id) REFERENCES conversation_branches (id)
        )",
        [],
    )?;
    Ok(())
}

fn create_table_branch_messages(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS branch_messages (
            id TEXT PRIMARY KEY,
            branch_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            sequence_number INTEGER NOT NULL,
            FOREIGN KEY (branch_id) REFERENCES conversation_branches (id),
            FOREIGN KEY (message_id) REFERENCES messages (id)
        )",
        [],
    )?;
    Ok(())
}

fn create_table_branch_metadata(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS branch_metadata (
            branch_id TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            PRIMARY KEY (branch_id, key),
            FOREIGN KEY (branch_id) REFERENCES conversation_branches (id)
        )",
        [],
    )?;
    Ok(())
}
