use super::entity::session_entity::SessionEntity;
use crate::session::entity::message_entity::MessageEntity;
use chrono::NaiveDateTime;
use std::fmt::Debug;

pub(crate) mod message_repository;
pub(crate) mod session_repository;
#[allow(dead_code)]
pub(crate) mod smart_context_repository;

pub trait SessionRepository
where
    Self::Error: Debug,
{
    type Error;

    fn fetch_all_sessions(&self) -> Result<Vec<SessionEntity>, Self::Error>;
    fn fetch_session_by_name(&self, name: &str) -> Result<SessionEntity, Self::Error>;
    fn add_session(
        &self,
        id: &str,
        name: &str,
        last_used_at: NaiveDateTime,
        current: bool,
    ) -> Result<(), Self::Error>;
    fn upsert_session(
        &self,
        id: &str,
        name: &str,
        last_used_at: NaiveDateTime,
        current: bool,
    ) -> Result<(), Self::Error>;
    fn remove_current_from_all(&self) -> Result<(), Self::Error>;
    fn delete_session(&self, session_id: &str) -> Result<(), Self::Error>;
}

pub trait MessageRepository
where
    Self::Error: Debug,
{
    type Error;

    fn fetch_messages_for_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<MessageEntity>, Self::Error>;
    fn add_message_to_session(&self, message: &MessageEntity) -> Result<(), Self::Error>;
    fn count_messages_for_session(&self, session_id: &str) -> Result<i64, Self::Error>;
    fn fetch_first_user_message(&self, session_id: &str) -> Result<Option<String>, Self::Error>;
    fn promote_message(&self, message_id: &str) -> Result<(), Self::Error>;
    fn fetch_pending_messages(&self, session_id: &str) -> Result<Vec<String>, Self::Error>;
    fn discard_pending_messages(&self, session_id: &str) -> Result<(), Self::Error>;
    fn search_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<message_repository::MessageMatch>, Self::Error>;
    fn fetch_conversation_rows(
        &self,
        session_id: &str,
    ) -> Result<Vec<message_repository::StoredRow>, Self::Error>;
    fn mark_duplicates(&self, ids: &[String]) -> Result<(), Self::Error>;
    fn restore_duplicates(&self) -> Result<usize, Self::Error>;
}
