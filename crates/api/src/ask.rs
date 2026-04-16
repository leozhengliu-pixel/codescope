use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sourcebot_core::AskThreadStore;
use sourcebot_models::{AskMessage, AskThread, AskThreadSummary, AskThreadVisibility};
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
}

#[allow(dead_code)]
pub fn build_ask_thread_store() -> DynAskThreadStore {
    Arc::new(InMemoryAskThreadStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sourcebot_models::{AskMessage, AskMessageRole, AskThreadVisibility};

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
}
