use crate::api::stream::StreamParser;
use crate::runtime::UiUpdate;
use crate::state::{ConversationManager, StreamBlock, ToolApprovalRequest, ToolStatus};
use crate::types::{ContentBlock, Delta, StreamEvent};
use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

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
            match client
                .create_stream_with_cancel(&messages, turn_cancel.clone())
                .await
            {
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

                            match event {
                                StreamEvent::ContentBlockStart {
                                    index,
                                    content_block,
                                } => {
                                    let block =
                                        content_block_to_stream_block(content_block.clone());
                                    let _ = tx.send(UiUpdate::StreamBlockStart { index, block });
                                    if let ContentBlock::ToolUse { name, input, .. } = content_block
                                    {
                                        let (response_tx, _response_rx) =
                                            oneshot::channel::<bool>();
                                        let _ = tx.send(UiUpdate::ToolApprovalRequest(
                                            ToolApprovalRequest {
                                                tool_name: name,
                                                input_preview: input.to_string(),
                                                response_tx,
                                            },
                                        ));
                                    }
                                }
                                StreamEvent::ContentBlockDelta {
                                    index,
                                    delta: delta @ Delta { text: Some(_), .. },
                                } => {
                                    if let Some(text) = delta.text.clone() {
                                        let _ = tx.send(UiUpdate::StreamBlockDelta {
                                            index,
                                            delta: text.clone(),
                                        });
                                        let _ = tx.send(UiUpdate::StreamDelta(text));
                                    }
                                }
                                StreamEvent::ContentBlockDelta {
                                    index,
                                    delta:
                                        Delta {
                                            partial_json: Some(partial_json),
                                            ..
                                        },
                                } => {
                                    let _ = tx.send(UiUpdate::StreamBlockDelta {
                                        index,
                                        delta: partial_json,
                                    });
                                }
                                StreamEvent::ContentBlockStop { index } => {
                                    let _ = tx.send(UiUpdate::StreamBlockComplete { index });
                                }
                                StreamEvent::MessageStop => {}
                                _ => {}
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

fn content_block_to_stream_block(content_block: ContentBlock) -> StreamBlock {
    match content_block {
        ContentBlock::Text { text } => StreamBlock::Thinking {
            content: text,
            collapsed: false,
        },
        ContentBlock::ToolUse { id, name, input } => StreamBlock::ToolCall {
            id,
            name,
            input,
            status: ToolStatus::Pending,
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => StreamBlock::ToolResult {
            tool_call_id: tool_use_id,
            output: content,
            is_error,
        },
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
    use std::time::Duration;
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
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
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

    #[test]
    fn test_ref_07_no_runtime_guard() {
        let (tx, mut rx) = mpsc::unbounded_channel::<UiUpdate>();
        let client = ApiClient::new_mock(Arc::new(MockApiClient::new(vec![])));
        let conversation = ConversationManager::new_mock(client, HashMap::new());
        let mut ctx = RuntimeContext::new(conversation, tx, CancellationToken::new());

        ctx.start_turn("test".to_string());

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

        assert!(
            ctx.conversation.messages_for_api().is_empty(),
            "history must stay clean when guard fires"
        );
    }

    #[tokio::test]
    async fn test_ref_08_start_turn_full_protocol_parity() {
        let chunks = vec![vec![
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n".to_string(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n".to_string(),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n".to_string(),
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n".to_string(),
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(),
        ]];

        let (tx, mut rx) = mpsc::unbounded_channel::<UiUpdate>();
        let client = ApiClient::new_mock(Arc::new(MockApiClient::new(chunks)));
        let conversation = ConversationManager::new_mock(client, HashMap::new());
        let mut ctx = RuntimeContext::new(conversation, tx, CancellationToken::new());

        ctx.start_turn("test".to_string());

        let mut events: Vec<&str> = vec![];
        loop {
            match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(UiUpdate::StreamBlockStart { .. })) => events.push("BlockStart"),
                Ok(Some(UiUpdate::StreamBlockDelta { .. })) => events.push("BlockDelta"),
                Ok(Some(UiUpdate::StreamBlockComplete { .. })) => events.push("BlockComplete"),
                Ok(Some(UiUpdate::StreamDelta(_))) => events.push("Delta"),
                Ok(Some(UiUpdate::TurnComplete)) => {
                    events.push("TurnComplete");
                    break;
                }
                Ok(Some(UiUpdate::Error(e))) => panic!("unexpected error: {e}"),
                _ => break,
            }
        }

        assert!(
            events.contains(&"TurnComplete"),
            "must terminate with TurnComplete"
        );
        assert_eq!(
            events.iter().filter(|&&e| e == "TurnComplete").count(),
            1,
            "exactly one TurnComplete"
        );
    }
}
