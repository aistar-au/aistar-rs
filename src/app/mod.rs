use crate::api::ApiClient;
use crate::config::Config;
use crate::runtime::context::RuntimeContext;
use crate::runtime::frontend::FrontendAdapter;
use crate::runtime::mode::RuntimeMode;
use crate::runtime::r#loop::Runtime;
use crate::runtime::UiUpdate;
use crate::state::ConversationManager;
use crate::tools::ToolExecutor;
use crate::ui::render::{input_visual_rows, render_input, render_messages, render_status_line};
use anyhow::Result;
use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend, layout::Constraint, layout::Direction, layout::Layout, Terminal,
};
use std::any::Any;
use std::io::Stdout;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const INTERRUPT_TOKEN: &str = "__AISTAR_INTERRUPT__";

pub struct TuiMode {
    history: Vec<String>,
    overlay_active: bool,
    turn_in_progress: bool,
}

impl TuiMode {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            overlay_active: false,
            turn_in_progress: false,
        }
    }

    fn status(&self) -> &'static str {
        if self.overlay_active {
            "awaiting tool approval"
        } else if self.turn_in_progress {
            "assistant is responding"
        } else {
            "ready"
        }
    }
}

impl Default for TuiMode {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeMode for TuiMode {
    fn on_user_input(&mut self, input: String, ctx: &mut RuntimeContext) {
        if input == INTERRUPT_TOKEN {
            if self.turn_in_progress {
                ctx.cancel_turn();
                self.turn_in_progress = false;
                self.overlay_active = false;
                self.history.push("[turn cancelled]".to_string());
            }
            return;
        }

        if self.overlay_active || self.turn_in_progress {
            return;
        }

        self.history.push(format!("> {input}"));
        self.turn_in_progress = true;
        ctx.start_turn(input);
    }

    fn on_model_update(&mut self, update: UiUpdate, _ctx: &mut RuntimeContext) {
        match update {
            UiUpdate::StreamDelta(text) => {
                if let Some(last) = self.history.last_mut() {
                    last.push_str(&text);
                } else {
                    self.history.push(text);
                }
            }
            UiUpdate::StreamBlockStart { .. }
            | UiUpdate::StreamBlockDelta { .. }
            | UiUpdate::StreamBlockComplete { .. } => {}
            UiUpdate::ToolApprovalRequest(request) => {
                self.overlay_active = true;
                self.history.push(format!(
                    "[tool approval requested: {}] auto-deny",
                    request.tool_name
                ));
                let _ = request.response_tx.send(false);
                self.overlay_active = false;
            }
            UiUpdate::TurnComplete => {
                self.turn_in_progress = false;
            }
            UiUpdate::Error(msg) => {
                self.history.push(format!("[error] {msg}"));
                self.turn_in_progress = false;
                self.overlay_active = false;
            }
        }
    }

    fn is_turn_in_progress(&self) -> bool {
        self.turn_in_progress
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct TuiFrontend {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    quit: bool,
    input_buffer: String,
}

impl TuiFrontend {
    pub fn new(terminal: Terminal<CrosstermBackend<Stdout>>) -> Self {
        Self {
            terminal,
            quit: false,
            input_buffer: String::new(),
        }
    }
}

impl FrontendAdapter for TuiFrontend {
    fn poll_user_input(&mut self) -> Option<String> {
        if poll(Duration::from_millis(16)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = read() {
                match key.code {
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.quit = true;
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Some(INTERRUPT_TOKEN.to_string());
                    }
                    KeyCode::Enter => {
                        let value = self.input_buffer.trim().to_string();
                        self.input_buffer.clear();
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                    }
                    KeyCode::Char(ch) => self.input_buffer.push(ch),
                    _ => {}
                }
            }
        }
        None
    }

    fn render<M: RuntimeMode>(&mut self, mode: &M) {
        let _ = self.terminal.draw(|frame| {
            let input_rows = input_visual_rows(&self.input_buffer, frame.area().width as usize)
                .clamp(1, 6) as u16;
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(input_rows),
                ])
                .split(frame.area());

            if let Some(tui_mode) = mode.as_any().downcast_ref::<TuiMode>() {
                render_messages(frame, chunks[0], &tui_mode.history, 0);
                render_status_line(frame, chunks[1], tui_mode.status());
            } else {
                render_messages(frame, chunks[0], &[], 0);
                render_status_line(frame, chunks[1], "runtime mode active");
            }

            render_input(
                frame,
                chunks[2],
                &self.input_buffer,
                self.input_buffer.len(),
            );
        });
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

pub struct App {
    runtime: Runtime<TuiMode>,
    frontend: TuiFrontend,
    ctx: RuntimeContext,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let client = ApiClient::new(&config)?;
        let executor = ToolExecutor::new(config.working_dir.clone());
        let conversation = ConversationManager::new(client, executor);

        let (update_tx, update_rx) = mpsc::unbounded_channel::<UiUpdate>();
        let ctx = RuntimeContext::new(conversation, update_tx, CancellationToken::new());

        let mode = TuiMode::new();
        let runtime = Runtime::new(mode, update_rx);

        let terminal = crate::terminal::setup()?;
        let frontend = TuiFrontend::new(terminal);

        Ok(Self {
            runtime,
            frontend,
            ctx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.runtime.run(&mut self.frontend, &mut self.ctx).await;
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = crate::terminal::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{mock_client::MockApiClient, ApiClient};
    use std::collections::HashMap;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_ref_03_tui_mode_overlay_blocks_input() {
        let (tx, _rx) = mpsc::unbounded_channel::<UiUpdate>();
        let client = ApiClient::new_mock(Arc::new(MockApiClient::new(vec![])));
        let conversation = ConversationManager::new_mock(client, HashMap::new());
        let mut ctx = RuntimeContext::new(conversation, tx, CancellationToken::new());

        let mut mode = TuiMode::new();
        mode.overlay_active = true;
        mode.on_user_input("blocked".to_string(), &mut ctx);

        assert!(!mode.turn_in_progress, "overlay must block input dispatch");
    }
}
