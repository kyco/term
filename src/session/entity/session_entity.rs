use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct SessionEntity {
    pub id: String,
    pub name: String,
    /// Legacy column, kept only so older binaries can still read this row.
    /// Sorting and display use `last_used_at`.
    pub expires_at: NaiveDateTime,
    pub current: i32,
    /// When the session was last opened or written to.
    pub last_used_at: NaiveDateTime,
}

impl SessionEntity {
    pub fn new(
        id: String,
        name: String,
        expires_at: NaiveDateTime,
        current: i32,
        last_used_at: NaiveDateTime,
    ) -> Self {
        Self {
            id,
            name,
            expires_at,
            current,
            last_used_at,
        }
    }
}
