use crate::api::{stream::StreamParser, ApiClient};
use crate::tools::ToolExecutor;
use crate::types::{ApiMessage, Content, ContentBlock, StreamEvent};
use anyhow::Result;
use futures::StreamExt;
use tokio::sync::mpsc;

pub struct ConversationManager {
    client: ApiClient,
    tool_executor: ToolExecutor,
    api_messages: Vec<ApiMessage>,
}

impl ConversationManager {
    pub fn new(client: ApiClient, executor: ToolExecutor) -> Self {
        Self {
            client,
            tool_executor: executor,
            api_messages: Vec::new(),
        }
    }

    pub async fn send_message(
        &mut self,
        content: String,
        stream_delta_tx: Option<&mpsc::UnboundedSender<String>>,
    ) -> Result<String> {
        self.api_messages.push(ApiMessage {
            role: "user".to_string(),
            content: Content::Text(content),
        });

        loop {
            let mut stream = self.client.create_stream(&self.api_messages).await?;
            let mut parser = StreamParser::new();
            let mut assistant_text = String::new();
            let mut tool_use_blocks = Vec::new();
            let mut tool_input_buffers: Vec<Option<String>> = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                let events = parser.process(&chunk)?;

                for event in events {
                    match event {
                        StreamEvent::ContentBlockStart {
                            index,
                            content_block,
                        } => {
                            if let ContentBlock::ToolUse { .. } = content_block {
                                while tool_use_blocks.len() <= index {
                                    tool_use_blocks.push(None);
                                    tool_input_buffers.push(None);
                                }
                                tool_use_blocks[index] = Some(content_block);
                                tool_input_buffers[index] = Some(String::new());
                            }
                        }
                        StreamEvent::ContentBlockDelta { index, delta } => {
                            if let Some(text) = delta.text {
                                assistant_text.push_str(&text);
                                if let Some(tx) = stream_delta_tx {
                                    let _ = tx.send(text);
                                }
                            }

                            if let Some(partial_json) = delta.partial_json {
                                let maybe_buffer = tool_input_buffers.get_mut(index);
                                if let Some(Some(buffer)) = maybe_buffer {
                                    buffer.push_str(&partial_json);
                                }
                            }
                        }
                        StreamEvent::ContentBlockStop { index } => {
                            let maybe_json = tool_input_buffers.get_mut(index);
                            let maybe_tool = tool_use_blocks.get_mut(index);

                            if let (
                                Some(Some(json_str)),
                                Some(Some(ContentBlock::ToolUse { input, .. })),
                            ) = (maybe_json, maybe_tool)
                            {
                                if !json_str.is_empty() {
                                    if let Ok(parsed_input) = serde_json::from_str(json_str) {
                                        *input = parsed_input;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            let tool_use_blocks: Vec<ContentBlock> =
                tool_use_blocks.into_iter().flatten().collect();

            let mut assistant_content_blocks = Vec::new();
            if !assistant_text.is_empty() {
                assistant_content_blocks.push(ContentBlock::Text {
                    text: assistant_text.clone(),
                });
            }
            assistant_content_blocks.extend(tool_use_blocks.clone());

            self.api_messages.push(ApiMessage {
                role: "assistant".to_string(),
                content: Content::Blocks(assistant_content_blocks),
            });

            if tool_use_blocks.is_empty() {
                return Ok(assistant_text);
            }

            let mut tool_result_blocks = Vec::new();
            for block in tool_use_blocks {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    let result = self.execute_tool(&name, &input).await;
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: result.as_ref().map_or_else(
                            |e| format!("Error executing tool: {e}"),
                            ToString::to_string,
                        ),
                        is_error: result.is_err(),
                    });
                }
            }

            self.api_messages.push(ApiMessage {
                role: "user".to_string(),
                content: Content::Blocks(tool_result_blocks),
            });
        }
    }

    async fn execute_tool(&self, name: &str, input: &serde_json::Value) -> Result<String> {
        match name {
            "read_file" => {
                let path = input["path"].as_str().unwrap_or("");
                self.tool_executor.read_file(path)
            }
            "write_file" => {
                let path = input["path"].as_str().unwrap_or("");
                let content = input["content"].as_str().unwrap_or("");
                self.tool_executor.write_file(path, content)?;
                Ok(format!("Successfully wrote to {path}"))
            }
            "edit_file" => {
                let path = input["path"].as_str().unwrap_or("");
                let old_str = input["old_str"].as_str().unwrap_or("");
                let new_str = input["new_str"].as_str().unwrap_or("");
                self.tool_executor.edit_file(path, old_str, new_str)?;
                Ok(format!("Successfully edited {path}"))
            }
            _ => Ok(format!("Unknown tool: {name}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ApiClient;
    use crate::config::Config;
    use crate::tools::ToolExecutor;

    #[test]
    fn test_crit_01_protocol_flow() {
        // ANCHOR: This test verifies the multi-turn conversation protocol.
        // It will FAIL until a mock API client is implemented.
        // 
        // The test should:
        // 1. Create a ConversationManager with a mock client
        // 2. Send a message that triggers tool use
        // 3. Verify the tool is executed
        // 4. Verify the final response incorporates tool results
        //
        // Current status: INCOMPLETE - needs mock ApiClient implementation
        
        // Placeholder assertion - this test needs the mock infrastructure
        assert!(false, "CRIT-01: Mock API client not yet implemented. See TASKS/CRIT-01-protocol.md");
    }
}
