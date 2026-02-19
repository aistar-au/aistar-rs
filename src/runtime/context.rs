use crate::api::stream::StreamParser;
use crate::runtime::UiUpdate;
use crate::state::ConversationManager;
use crate::types::StreamEvent;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Capability surface passed to `RuntimeMode` methods.
///
/// Owns `ConversationManager` (not a borrow) so that REF-05's runtime loop
/// can hold it without a lifetime parameter. See ADR-006 §2.
pub struct RuntimeContext {
    pub(crate) conversation: ConversationManager,
    pub(crate) update_tx: mpsc::UnboundedSender<UiUpdate>,
    pub(crate) cancel: CancellationToken,
}

impl RuntimeContext {
    pub fn new(
        conversation: ConversationManager,
        update_tx: mpsc::UnboundedSender<UiUpdate>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            conversation,
            update_tx,
            cancel,
        }
    }

    pub fn start_turn(&mut self, input: String) {
        // Guard: refuse to spawn without an active Tokio runtime.
        // Must precede push_user_message so history stays clean on error path.
        if tokio::runtime::Handle::try_current().is_err() {
            let _ = self.update_tx.send(UiUpdate::Error(
                "runtime error: start_turn requires active Tokio runtime".to_string(),
            ));
            return;
        }

        self.conversation.push_user_message(input);

        let turn_cancel = self.cancel.child_token();
        let tx = self.update_tx.clone();
        let messages = self.conversation.messages_for_api();
        let client = self.conversation.client();

        tokio::spawn(async move {
            let result = client.create_stream_with_cancel(&messages, turn_cancel.clone()).await;

            match result {
                Ok(mut stream) => {
                    let mut parser = StreamParser::new();
                    while let Some(chunk_result) = stream.next().await {
                        if turn_cancel.is_cancelled() {
                            break;
                        }

                        let chunk = match chunk_result {
                            Ok(chunk) => chunk,
                            Err(e) => {
                                let _ = tx.send(UiUpdate::Error(e.to_string()));
                                return;
                            }
                        };

                        let events = match parser.process(&chunk) {
                            Ok(events) => events,
                            Err(e) => {
                                let _ = tx.send(UiUpdate::Error(e.to_string()));
                                return;
                            }
                        };

                        for event in events {
                            if turn_cancel.is_cancelled() {
                                break;
                            }
                            if let StreamEvent::ContentBlockDelta {
                                delta: crate::types::Delta { text: Some(text), .. },
                                ..
                            } = event
                            {
                                let _ = tx.send(UiUpdate::StreamDelta(text));
                            }
                        }
                    }
                    let _ = tx.send(UiUpdate::TurnComplete);
                }
                Err(e) => {
                    let _ = tx.send(UiUpdate::Error(e.to_string()));
                }
            }
        });
    }

    pub fn cancel_turn(&mut self) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeContext;
    use crate::api::{mock_client::MockApiClient, ApiClient};
    use crate::runtime::UiUpdate;
    use crate::state::ConversationManager;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_ref_04_start_turn_dispatches_message() {
        let (tx, mut rx) = mpsc::unbounded_channel::<UiUpdate>();

        let client = ApiClient::new_mock(Arc::new(MockApiClient::new(vec![vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n".to_string(),
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}]}\n\n".to_string(),
        ]])));
        let conversation = ConversationManager::new_mock(client, HashMap::new());

        let mut ctx = RuntimeContext::new(conversation, tx, CancellationToken::new());

        ctx.start_turn("test input".to_string());

        let mut saw_delta = false;
        let mut saw_complete = false;
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await {
                Ok(Some(UiUpdate::StreamDelta(_))) => saw_delta = true,
                Ok(Some(UiUpdate::TurnComplete)) => {
                    saw_complete = true;
                    break;
                }
                Ok(Some(UiUpdate::Error(e))) => panic!("unexpected error: {e}"),
                Ok(None) | Err(_) => break,
                _ => {}
            }
        }

        assert!(saw_delta, "expected at least one StreamDelta");
        assert!(saw_complete, "expected TurnComplete");
    }

    /// REF-07: calling start_turn without a Tokio runtime must not panic.
    /// Emits UiUpdate::Error and leaves conversation history untouched.
    #[test]
    fn test_ref_07_no_runtime_guard() {
        let (tx, mut rx) = mpsc::unbounded_channel::<UiUpdate>();
        let client = ApiClient::new_mock(Arc::new(MockApiClient::new(vec![])));
        let conversation = ConversationManager::new_mock(client, HashMap::new());
        let mut ctx = RuntimeContext::new(conversation, tx, CancellationToken::new());

        // No #[tokio::test] — no runtime is active.
        ctx.start_turn("test".to_string());

        // Must emit an error, not spawn.
        let update = rx.try_recv().expect("expected error update");
        match update {
            UiUpdate::Error(msg) => {
                assert!(
                    msg.contains("requires active Tokio runtime"),
                    "unexpected error message: {msg}"
                );
            }
            _ => panic!("expected UiUpdate::Error, got something else"),
        }

        // No message appended to history on guard failure.
        assert!(
            ctx.conversation.messages_for_api().is_empty(),
            "history must stay clean when guard fires"
        );
    }
}
