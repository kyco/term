#[derive(Debug, Clone)]
pub struct MessageEntity {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub message_type: String,
    pub compaction_metadata: Option<String>,
}

impl MessageEntity {
    pub fn new(id: String, session_id: String, role: String, content: String) -> Self {
        Self {
            id,
            session_id,
            role,
            content,
            message_type: "standard".to_string(),
            compaction_metadata: None,
        }
    }

    pub fn new_with_type(
        id: String,
        session_id: String,
        role: String,
        content: String,
        message_type: String,
        compaction_metadata: Option<String>,
    ) -> Self {
        Self {
            id,
            session_id,
            role,
            content,
            message_type,
            compaction_metadata,
        }
    }
}
