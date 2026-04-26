use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sourcebot_core::{AskRequest, AskThreadStore};
use sourcebot_models::{
    AskCitation, AskMessage, AskMessageRole, AskRenderedCitation, AskThread, AskThreadSummary,
    AskThreadVisibility,
};
use std::path::{Component, Path};
use std::sync::{Arc, RwLock};

#[allow(dead_code)]
pub type DynAskThreadStore = Arc<dyn AskThreadStore>;

#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct InMemoryAskThreadStore {
    threads: Arc<RwLock<Vec<AskThread>>>,
}

impl InMemoryAskThreadStore {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AskThreadStore for InMemoryAskThreadStore {
    async fn create_thread(&self, thread: AskThread) -> Result<()> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        if threads.iter().any(|existing| existing.id == thread.id) {
            anyhow::bail!("ask thread {} already exists", thread.id);
        }

        threads.push(thread);
        Ok(())
    }

    async fn list_threads_for_user(&self, user_id: &str) -> Result<Vec<AskThreadSummary>> {
        let threads = self
            .threads
            .read()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let mut summaries: Vec<_> = threads
            .iter()
            .filter(|thread| thread.user_id == user_id)
            .map(AskThread::summary)
            .collect();

        summaries.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });

        Ok(summaries)
    }

    async fn get_thread_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Option<AskThread>> {
        let threads = self
            .threads
            .read()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        Ok(threads
            .iter()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
            .cloned())
    }

    async fn get_thread_messages_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Option<Vec<AskMessage>>> {
        let threads = self
            .threads
            .read()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        Ok(threads
            .iter()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
            .map(|thread| thread.messages.clone()))
    }

    async fn get_thread_for_session_for_user(
        &self,
        user_id: &str,
        session_id: &str,
    ) -> Result<Option<AskThread>> {
        let threads = self
            .threads
            .read()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        Ok(threads
            .iter()
            .find(|thread| thread.user_id == user_id && thread.session_id == session_id)
            .cloned())
    }

    async fn update_thread_metadata_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        title: Option<&str>,
        visibility: Option<AskThreadVisibility>,
        updated_at: &str,
    ) -> Result<Option<AskThread>> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let Some(thread) = threads
            .iter_mut()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
        else {
            return Ok(None);
        };

        if let Some(title) = title {
            thread.title = title.into();
        }

        if let Some(visibility) = visibility {
            thread.visibility = visibility;
        }

        thread.updated_at = updated_at.into();

        Ok(Some(thread.clone()))
    }

    async fn append_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message: AskMessage,
        updated_at: &str,
    ) -> Result<Option<AskThread>> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let Some(thread) = threads
            .iter_mut()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
        else {
            return Ok(None);
        };

        thread.messages.push(message);
        thread.updated_at = updated_at.into();

        Ok(Some(thread.clone()))
    }

    async fn update_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        content: &str,
        updated_at: &str,
    ) -> Result<Option<AskThread>> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let Some(thread) = threads
            .iter_mut()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
        else {
            return Ok(None);
        };

        let Some(message) = thread
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
        else {
            return Ok(None);
        };

        message.content = content.into();
        thread.updated_at = updated_at.into();

        Ok(Some(thread.clone()))
    }

    async fn replace_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        message: AskMessage,
        updated_at: &str,
    ) -> Result<Option<AskThread>> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let Some(thread) = threads
            .iter_mut()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
        else {
            return Ok(None);
        };

        let Some(existing_message) = thread
            .messages
            .iter_mut()
            .find(|existing_message| existing_message.id == message_id)
        else {
            return Ok(None);
        };

        *existing_message = message;
        thread.updated_at = updated_at.into();

        Ok(Some(thread.clone()))
    }

    async fn delete_message_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
        message_id: &str,
        updated_at: &str,
    ) -> Result<Option<AskThread>> {
        let mut threads = self
            .threads
            .write()
            .map_err(|_| anyhow!("ask thread store lock poisoned"))?;

        let Some(thread) = threads
            .iter_mut()
            .find(|thread| thread.user_id == user_id && thread.id == thread_id)
        else {
            return Ok(None);
        };

        let Some(message_index) = thread
            .messages
            .iter()
            .position(|message| message.id == message_id)
        else {
            return Ok(None);
        };

        thread.messages.remove(message_index);
        thread.updated_at = updated_at.into();

        Ok(Some(thread.clone()))
    }
}

#[allow(dead_code)]
pub fn build_ask_thread_store() -> DynAskThreadStore {
    Arc::new(InMemoryAskThreadStore::new())
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AskMessageResponse {
    pub id: String,
    pub role: AskMessageRole,
    pub content: String,
    pub citations: Vec<AskCitation>,
    pub rendered_citations: Vec<AskRenderedCitation>,
}

impl From<&AskMessage> for AskMessageResponse {
    fn from(message: &AskMessage) -> Self {
        Self {
            id: message.id.clone(),
            role: message.role.clone(),
            content: message.content.clone(),
            citations: message.citations.clone(),
            rendered_citations: message
                .citations
                .iter()
                .map(|citation| citation.rendered())
                .collect(),
        }
    }
}

impl From<AskMessage> for AskMessageResponse {
    fn from(message: AskMessage) -> Self {
        Self::from(&message)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AskThreadResponse {
    pub id: String,
    pub session_id: String,
    pub user_id: String,
    pub title: String,
    pub repo_scope: Vec<String>,
    pub visibility: AskThreadVisibility,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<AskMessageResponse>,
}

impl From<&AskThread> for AskThreadResponse {
    fn from(thread: &AskThread) -> Self {
        Self {
            id: thread.id.clone(),
            session_id: thread.session_id.clone(),
            user_id: thread.user_id.clone(),
            title: thread.title.clone(),
            repo_scope: thread.repo_scope.clone(),
            visibility: thread.visibility.clone(),
            created_at: thread.created_at.clone(),
            updated_at: thread.updated_at.clone(),
            messages: thread
                .messages
                .iter()
                .map(AskMessageResponse::from)
                .collect(),
        }
    }
}

impl From<AskThread> for AskThreadResponse {
    fn from(thread: AskThread) -> Self {
        Self::from(&thread)
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AskCompletionRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub repo_scope: Vec<String>,
    pub thread_id: Option<String>,
}

impl AskCompletionRequest {
    pub fn into_core_request(self, known_repo_ids: &[String]) -> Result<AskRequest, StatusCode> {
        let prompt = self.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }

        let repo_scope = canonicalize_repo_scope(self.repo_scope);
        if repo_scope.is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }

        if repo_scope.iter().any(|repo_id| {
            !known_repo_ids
                .iter()
                .any(|known_repo_id| known_repo_id == repo_id)
        }) {
            return Err(StatusCode::BAD_REQUEST);
        }

        Ok(AskRequest {
            prompt,
            system_prompt: self.system_prompt,
            repo_scope,
            thread_id: self.thread_id,
            previous_messages: Vec::new(),
        })
    }
}

pub fn canonicalize_repo_scope(repo_scope: Vec<String>) -> Vec<String> {
    let mut repo_scope: Vec<String> = repo_scope
        .into_iter()
        .map(|repo_id| repo_id.trim().to_string())
        .filter(|repo_id| !repo_id.is_empty())
        .collect();
    repo_scope.sort();
    repo_scope.dedup();
    repo_scope
}

fn citation_path_is_safe(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }

    let candidate = Path::new(trimmed);
    if candidate.is_absolute() {
        return false;
    }

    !candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AskCompletionResponse {
    pub provider: String,
    pub model: Option<String>,
    pub answer: String,
    pub citations: Vec<AskCitation>,
    pub rendered_citations: Vec<AskRenderedCitation>,
    pub thread_id: String,
    pub session_id: String,
}

pub fn filter_ask_citations_to_repo_scope(
    citations: &[AskCitation],
    repo_scope: &[String],
) -> Vec<AskCitation> {
    citations
        .iter()
        .filter(|citation| {
            repo_scope
                .iter()
                .any(|repo_id| repo_id == &citation.repo_id)
                && citation_path_is_safe(&citation.path)
                && !citation.revision.trim().is_empty()
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{AskCitation, AskMessage, AskMessageRole, AskThreadVisibility};

    fn citation(path: &str, revision: &str, line_start: usize, line_end: usize) -> AskCitation {
        AskCitation {
            repo_id: "repo_sourcebot_rewrite".into(),
            path: path.into(),
            revision: revision.into(),
            line_start,
            line_end,
        }
    }

    fn thread(
        id: &str,
        user_id: &str,
        updated_at: &str,
        title: &str,
        session_id: &str,
    ) -> AskThread {
        AskThread {
            id: id.into(),
            session_id: session_id.into(),
            user_id: user_id.into(),
            title: title.into(),
            repo_scope: vec!["repo_sourcebot_rewrite".into()],
            visibility: AskThreadVisibility::Private,
            created_at: "2026-04-16T08:00:00Z".into(),
            updated_at: updated_at.into(),
            messages: vec![AskMessage {
                id: format!("msg_{id}"),
                role: AskMessageRole::User,
                content: "where is healthz implemented?".into(),
                citations: Vec::new(),
            }],
        }
    }

    #[tokio::test]
    async fn build_ask_thread_store_starts_empty() {
        let store = build_ask_thread_store();

        assert_eq!(
            store.list_threads_for_user("user_1").await.unwrap(),
            Vec::new()
        );
    }

    #[tokio::test]
    async fn in_memory_store_lists_threads_for_owner_in_recent_first_order() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_older",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Older thread",
                "session_a",
            ))
            .await
            .unwrap();
        store
            .create_thread(thread(
                "thread_newer",
                "user_1",
                "2026-04-16T08:02:00Z",
                "Newer thread",
                "session_b",
            ))
            .await
            .unwrap();

        let threads = store.list_threads_for_user("user_1").await.unwrap();

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "thread_newer");
        assert_eq!(threads[0].message_count, 1);
        assert_eq!(threads[1].id, "thread_older");
    }

    #[tokio::test]
    async fn in_memory_store_returns_full_thread_only_to_owner() {
        let store = build_ask_thread_store();
        let expected = thread(
            "thread_private",
            "user_1",
            "2026-04-16T08:02:00Z",
            "Private thread",
            "session_a",
        );
        store.create_thread(expected.clone()).await.unwrap();

        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private")
                .await
                .unwrap(),
            Some(expected)
        );
        assert_eq!(
            store
                .get_thread_for_user("user_2", "thread_private")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn in_memory_store_rejects_duplicate_thread_ids() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_duplicate",
                "user_1",
                "2026-04-16T08:01:00Z",
                "First thread",
                "session_a",
            ))
            .await
            .unwrap();

        let err = store
            .create_thread(thread(
                "thread_duplicate",
                "user_1",
                "2026-04-16T08:03:00Z",
                "Second thread",
                "session_b",
            ))
            .await
            .expect_err("duplicate thread ids should be rejected");

        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn in_memory_store_updates_thread_title_and_visibility_for_owner() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_mutable",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Original title",
                "session_a",
            ))
            .await
            .unwrap();

        let updated = store
            .update_thread_metadata_for_user(
                "user_1",
                "thread_mutable",
                Some("Renamed thread"),
                Some(AskThreadVisibility::Shared),
                "2026-04-16T08:05:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to update metadata");

        assert_eq!(updated.title, "Renamed thread");
        assert_eq!(updated.visibility, AskThreadVisibility::Shared);
        assert_eq!(updated.updated_at, "2026-04-16T08:05:00Z");

        let summaries = store.list_threads_for_user("user_1").await.unwrap();
        assert_eq!(summaries[0].title, "Renamed thread");
        assert_eq!(summaries[0].visibility, AskThreadVisibility::Shared);
        assert_eq!(summaries[0].updated_at, "2026-04-16T08:05:00Z");
    }

    #[tokio::test]
    async fn in_memory_store_does_not_update_thread_metadata_for_non_owner() {
        let store = build_ask_thread_store();
        let original = thread(
            "thread_private",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Original title",
            "session_a",
        );
        store.create_thread(original.clone()).await.unwrap();

        let updated = store
            .update_thread_metadata_for_user(
                "user_2",
                "thread_private",
                Some("Hacked title"),
                Some(AskThreadVisibility::Shared),
                "2026-04-16T08:05:00Z",
            )
            .await
            .unwrap();

        assert_eq!(updated, None);
        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private")
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[tokio::test]
    async fn in_memory_store_returns_thread_for_matching_user_session() {
        let store = build_ask_thread_store();
        let expected = thread(
            "thread_session_linked",
            "user_1",
            "2026-04-16T08:03:00Z",
            "Session linked thread",
            "session_linked",
        );
        store.create_thread(expected.clone()).await.unwrap();
        store
            .create_thread(thread(
                "thread_other_user",
                "user_2",
                "2026-04-16T08:04:00Z",
                "Other user's thread",
                "session_linked",
            ))
            .await
            .unwrap();

        assert_eq!(
            store
                .get_thread_for_session_for_user("user_1", "session_linked")
                .await
                .unwrap(),
            Some(expected)
        );
        assert_eq!(
            store
                .get_thread_for_session_for_user("user_1", "missing_session")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn in_memory_store_appends_message_for_owner_and_updates_summary() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_appendable",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Appendable thread",
                "session_a",
            ))
            .await
            .unwrap();

        let updated = store
            .append_message_for_user(
                "user_1",
                "thread_appendable",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "healthz lives in crates/api/src/main.rs".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append a message");

        assert_eq!(updated.messages.len(), 2);
        assert_eq!(updated.messages[1].id, "msg_assistant");
        assert_eq!(updated.updated_at, "2026-04-16T08:06:00Z");

        let summaries = store.list_threads_for_user("user_1").await.unwrap();
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].updated_at, "2026-04-16T08:06:00Z");
    }

    #[tokio::test]
    async fn in_memory_store_does_not_append_message_for_non_owner() {
        let store = build_ask_thread_store();
        let original = thread(
            "thread_private",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Private thread",
            "session_a",
        );
        store.create_thread(original.clone()).await.unwrap();

        let updated = store
            .append_message_for_user(
                "user_2",
                "thread_private",
                AskMessage {
                    id: "msg_intruder".into(),
                    role: AskMessageRole::Assistant,
                    content: "unauthorized".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap();

        assert_eq!(updated, None);
        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private")
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[tokio::test]
    async fn in_memory_store_returns_thread_messages_for_owner_in_append_order() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_messages",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Messages thread",
                "session_a",
            ))
            .await
            .unwrap();
        store
            .append_message_for_user(
                "user_1",
                "thread_messages",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "healthz lives in crates/api/src/main.rs".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append a message");

        let messages = store
            .get_thread_messages_for_user("user_1", "thread_messages")
            .await
            .unwrap()
            .expect("owner should be able to read persisted messages");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, "msg_thread_messages");
        assert_eq!(messages[0].role, AskMessageRole::User);
        assert_eq!(messages[1].id, "msg_assistant");
        assert_eq!(messages[1].role, AskMessageRole::Assistant);
    }

    #[tokio::test]
    async fn in_memory_store_hides_thread_messages_from_non_owner_and_missing_threads() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_private_messages",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Private messages",
                "session_a",
            ))
            .await
            .unwrap();

        assert_eq!(
            store
                .get_thread_messages_for_user("user_2", "thread_private_messages")
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .get_thread_messages_for_user("user_1", "missing_thread")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn in_memory_store_updates_message_content_for_owner() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_editable_messages",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Editable messages",
                "session_a",
            ))
            .await
            .unwrap();
        store
            .append_message_for_user(
                "user_1",
                "thread_editable_messages",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "draft answer".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append a message before updating it");

        let updated = store
            .update_message_for_user(
                "user_1",
                "thread_editable_messages",
                "msg_assistant",
                "final answer with citations",
                "2026-04-16T08:07:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to update a persisted message");

        assert_eq!(updated.updated_at, "2026-04-16T08:07:00Z");
        assert_eq!(updated.messages.len(), 2);
        assert_eq!(updated.messages[1].id, "msg_assistant");
        assert_eq!(updated.messages[1].content, "final answer with citations");

        let messages = store
            .get_thread_messages_for_user("user_1", "thread_editable_messages")
            .await
            .unwrap()
            .expect("owner should be able to reload persisted messages");
        assert_eq!(messages[1].content, "final answer with citations");

        let summaries = store.list_threads_for_user("user_1").await.unwrap();
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].updated_at, "2026-04-16T08:07:00Z");
    }

    #[tokio::test]
    async fn in_memory_store_does_not_update_message_for_non_owner_or_missing_message() {
        let store = build_ask_thread_store();
        let original = thread(
            "thread_private_messages",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Private messages",
            "session_a",
        );
        store.create_thread(original.clone()).await.unwrap();

        assert_eq!(
            store
                .update_message_for_user(
                    "user_2",
                    "thread_private_messages",
                    "msg_thread_private_messages",
                    "unauthorized",
                    "2026-04-16T08:07:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .update_message_for_user(
                    "user_1",
                    "thread_private_messages",
                    "missing_message",
                    "updated",
                    "2026-04-16T08:07:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private_messages")
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[tokio::test]
    async fn in_memory_store_deletes_message_for_owner_and_updates_summary() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_deletable_messages",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Deletable messages",
                "session_a",
            ))
            .await
            .unwrap();
        store
            .append_message_for_user(
                "user_1",
                "thread_deletable_messages",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "assistant reply".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append a message before deleting it");

        let updated = store
            .delete_message_for_user(
                "user_1",
                "thread_deletable_messages",
                "msg_thread_deletable_messages",
                "2026-04-16T08:08:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to delete a persisted message");

        assert_eq!(updated.updated_at, "2026-04-16T08:08:00Z");
        assert_eq!(updated.messages.len(), 1);
        assert_eq!(updated.messages[0].id, "msg_assistant");

        let messages = store
            .get_thread_messages_for_user("user_1", "thread_deletable_messages")
            .await
            .unwrap()
            .expect("owner should be able to reload persisted messages after deletion");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "msg_assistant");

        let summaries = store.list_threads_for_user("user_1").await.unwrap();
        assert_eq!(summaries[0].message_count, 1);
        assert_eq!(summaries[0].updated_at, "2026-04-16T08:08:00Z");
    }

    #[tokio::test]
    async fn in_memory_store_does_not_delete_message_for_non_owner_missing_thread_or_missing_message(
    ) {
        let store = build_ask_thread_store();
        let original = thread(
            "thread_private_messages",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Private messages",
            "session_a",
        );
        store.create_thread(original.clone()).await.unwrap();

        assert_eq!(
            store
                .delete_message_for_user(
                    "user_2",
                    "thread_private_messages",
                    "msg_thread_private_messages",
                    "2026-04-16T08:08:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .delete_message_for_user(
                    "user_1",
                    "missing_thread",
                    "msg_thread_private_messages",
                    "2026-04-16T08:08:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .delete_message_for_user(
                    "user_1",
                    "thread_private_messages",
                    "missing_message",
                    "2026-04-16T08:08:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private_messages")
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[tokio::test]
    async fn in_memory_store_replaces_message_for_owner_without_reordering_messages() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_replaceable_messages",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Replaceable messages",
                "session_a",
            ))
            .await
            .unwrap();
        store
            .append_message_for_user(
                "user_1",
                "thread_replaceable_messages",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "draft assistant reply".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append a message before replacing it");

        let updated = store
            .replace_message_for_user(
                "user_1",
                "thread_replaceable_messages",
                "msg_assistant",
                AskMessage {
                    id: "msg_assistant_replaced".into(),
                    role: AskMessageRole::Assistant,
                    content: "final assistant reply with citations".into(),
                    citations: Vec::new(),
                },
                "2026-04-16T08:09:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to replace a persisted message");

        assert_eq!(updated.updated_at, "2026-04-16T08:09:00Z");
        assert_eq!(updated.messages.len(), 2);
        assert_eq!(updated.messages[0].id, "msg_thread_replaceable_messages");
        assert_eq!(updated.messages[1].id, "msg_assistant_replaced");
        assert_eq!(
            updated.messages[1].content,
            "final assistant reply with citations"
        );

        let messages = store
            .get_thread_messages_for_user("user_1", "thread_replaceable_messages")
            .await
            .unwrap()
            .expect("owner should be able to reload persisted messages after replacement");
        assert_eq!(messages[0].id, "msg_thread_replaceable_messages");
        assert_eq!(messages[1].id, "msg_assistant_replaced");
        assert_eq!(messages[1].content, "final assistant reply with citations");

        let summaries = store.list_threads_for_user("user_1").await.unwrap();
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].updated_at, "2026-04-16T08:09:00Z");
    }

    #[tokio::test]
    async fn in_memory_store_does_not_replace_message_for_non_owner_missing_thread_or_missing_message(
    ) {
        let store = build_ask_thread_store();
        let original = thread(
            "thread_private_messages",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Private messages",
            "session_a",
        );
        store.create_thread(original.clone()).await.unwrap();

        assert_eq!(
            store
                .replace_message_for_user(
                    "user_2",
                    "thread_private_messages",
                    "msg_thread_private_messages",
                    AskMessage {
                        id: "msg_replaced".into(),
                        role: AskMessageRole::Assistant,
                        content: "unauthorized".into(),
                        citations: Vec::new(),
                    },
                    "2026-04-16T08:09:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .replace_message_for_user(
                    "user_1",
                    "missing_thread",
                    "msg_thread_private_messages",
                    AskMessage {
                        id: "msg_replaced".into(),
                        role: AskMessageRole::Assistant,
                        content: "missing thread".into(),
                        citations: Vec::new(),
                    },
                    "2026-04-16T08:09:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .replace_message_for_user(
                    "user_1",
                    "thread_private_messages",
                    "missing_message",
                    AskMessage {
                        id: "msg_replaced".into(),
                        role: AskMessageRole::Assistant,
                        content: "missing message".into(),
                        citations: Vec::new(),
                    },
                    "2026-04-16T08:09:00Z",
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .get_thread_for_user("user_1", "thread_private_messages")
                .await
                .unwrap(),
            Some(original)
        );
    }

    #[test]
    fn ask_message_response_includes_rendered_citations_alongside_raw_citations() {
        let message = AskMessage {
            id: "msg_assistant".into(),
            role: AskMessageRole::Assistant,
            content: "healthz lives in crates/api/src/main.rs".into(),
            citations: vec![citation("crates/api/src/main.rs", "main", 10, 18)],
        };

        let response = AskMessageResponse::from(&message);

        assert_eq!(response.id, message.id);
        assert_eq!(response.role, message.role);
        assert_eq!(response.content, message.content);
        assert_eq!(response.citations, message.citations);
        assert_eq!(response.rendered_citations.len(), 1);
        assert_eq!(
            response.rendered_citations[0],
            message.citations[0].rendered()
        );
    }

    #[test]
    fn ask_thread_response_maps_message_rendered_citations() {
        let mut thread = thread(
            "thread_citations",
            "user_1",
            "2026-04-16T08:01:00Z",
            "Citation thread",
            "session_a",
        );
        thread.messages.push(AskMessage {
            id: "msg_assistant".into(),
            role: AskMessageRole::Assistant,
            content: "healthz lives in crates/api/src/main.rs".into(),
            citations: vec![citation("crates/api/src/main.rs", "main", 10, 18)],
        });

        let response = AskThreadResponse::from(&thread);

        assert_eq!(response.id, thread.id);
        assert_eq!(response.session_id, thread.session_id);
        assert_eq!(response.user_id, thread.user_id);
        assert_eq!(response.title, thread.title);
        assert_eq!(response.repo_scope, thread.repo_scope);
        assert_eq!(response.visibility, thread.visibility);
        assert_eq!(response.created_at, thread.created_at);
        assert_eq!(response.updated_at, thread.updated_at);
        assert_eq!(response.messages.len(), thread.messages.len());
        assert_eq!(response.messages[1].citations, thread.messages[1].citations);
        assert_eq!(
            response.messages[1].rendered_citations,
            vec![thread.messages[1].citations[0].rendered()]
        );
    }

    #[test]
    fn ask_completion_response_includes_rendered_citations_alongside_raw_citations() {
        let completion = sourcebot_core::AskResponse {
            provider: "openai".into(),
            model: Some("gpt-4.1-mini".into()),
            answer: "healthz lives in crates/api/src/main.rs".into(),
            citations: vec![citation("crates/api/src/main.rs", "main", 10, 18)],
        };

        let response = AskCompletionResponse {
            provider: completion.provider.clone(),
            model: completion.model.clone(),
            answer: completion.answer.clone(),
            citations: completion.citations.clone(),
            rendered_citations: completion
                .citations
                .iter()
                .map(|citation| citation.rendered())
                .collect(),
            thread_id: "thread_123".into(),
            session_id: "session_123".into(),
        };

        assert_eq!(response.provider, completion.provider);
        assert_eq!(response.model, completion.model);
        assert_eq!(response.answer, completion.answer);
        assert_eq!(response.citations, completion.citations);
        assert_eq!(response.rendered_citations.len(), 1);
        assert_eq!(
            response.rendered_citations[0],
            completion.citations[0].rendered()
        );
        assert_eq!(response.thread_id, "thread_123");
        assert_eq!(response.session_id, "session_123");
    }

    #[test]
    fn filter_ask_citations_to_repo_scope_excludes_out_of_scope_repositories() {
        let citations = vec![
            sourcebot_models::AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "crates/api/src/main.rs".into(),
                revision: "main".into(),
                line_start: 10,
                line_end: 18,
            },
            sourcebot_models::AskCitation {
                repo_id: "repo_hidden".into(),
                path: "secret.txt".into(),
                revision: "deadbeef".into(),
                line_start: 1,
                line_end: 1,
            },
        ];

        let filtered =
            filter_ask_citations_to_repo_scope(&citations, &["repo_sourcebot_rewrite".into()]);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].repo_id, "repo_sourcebot_rewrite");
        assert_eq!(filtered[0].path, "crates/api/src/main.rs");
    }

    #[test]
    fn filter_ask_citations_to_repo_scope_excludes_unsafe_paths_and_blank_revisions() {
        let citations = vec![
            sourcebot_models::AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "../secrets.txt".into(),
                revision: "main".into(),
                line_start: 1,
                line_end: 2,
            },
            sourcebot_models::AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "/etc/passwd".into(),
                revision: "main".into(),
                line_start: 1,
                line_end: 1,
            },
            sourcebot_models::AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src/lib.rs".into(),
                revision: "   ".into(),
                line_start: 5,
                line_end: 9,
            },
            sourcebot_models::AskCitation {
                repo_id: "repo_sourcebot_rewrite".into(),
                path: "src/main.rs".into(),
                revision: "main".into(),
                line_start: 10,
                line_end: 12,
            },
        ];

        let filtered =
            filter_ask_citations_to_repo_scope(&citations, &["repo_sourcebot_rewrite".into()]);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, "src/main.rs");
        assert_eq!(filtered[0].revision, "main");
    }

    #[test]
    fn canonicalize_repo_scope_sorts_and_deduplicates_trimmed_entries() {
        assert_eq!(
            canonicalize_repo_scope(vec![
                " repo_b ".into(),
                "repo_a".into(),
                "repo_b".into(),
                "".into(),
            ]),
            vec!["repo_a".to_string(), "repo_b".to_string()]
        );
    }

    #[tokio::test]
    async fn in_memory_store_preserves_message_citations_across_reload_and_replace() {
        let store = build_ask_thread_store();
        store
            .create_thread(thread(
                "thread_citations",
                "user_1",
                "2026-04-16T08:01:00Z",
                "Citation thread",
                "session_a",
            ))
            .await
            .unwrap();

        store
            .append_message_for_user(
                "user_1",
                "thread_citations",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "healthz lives in crates/api/src/main.rs".into(),
                    citations: vec![sourcebot_models::AskCitation {
                        repo_id: "repo_sourcebot_rewrite".into(),
                        path: "crates/api/src/main.rs".into(),
                        revision: "main".into(),
                        line_start: 10,
                        line_end: 18,
                    }],
                },
                "2026-04-16T08:06:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to append cited message");

        let appended_messages = store
            .get_thread_messages_for_user("user_1", "thread_citations")
            .await
            .unwrap()
            .expect("owner should be able to reload cited messages");
        assert_eq!(appended_messages[1].citations.len(), 1);
        assert_eq!(
            appended_messages[1].citations[0].path,
            "crates/api/src/main.rs"
        );

        store
            .replace_message_for_user(
                "user_1",
                "thread_citations",
                "msg_assistant",
                AskMessage {
                    id: "msg_assistant".into(),
                    role: AskMessageRole::Assistant,
                    content: "updated answer".into(),
                    citations: vec![sourcebot_models::AskCitation {
                        repo_id: "repo_sourcebot_rewrite".into(),
                        path: "crates/api/src/ask.rs".into(),
                        revision: "main".into(),
                        line_start: 20,
                        line_end: 40,
                    }],
                },
                "2026-04-16T08:09:00Z",
            )
            .await
            .unwrap()
            .expect("owner should be able to replace cited message");

        let replaced_messages = store
            .get_thread_messages_for_user("user_1", "thread_citations")
            .await
            .unwrap()
            .expect("owner should be able to reload replaced cited messages");
        assert_eq!(replaced_messages[1].citations.len(), 1);
        assert_eq!(
            replaced_messages[1].citations[0].path,
            "crates/api/src/ask.rs"
        );
        assert_eq!(replaced_messages[1].content, "updated answer");
    }
}
