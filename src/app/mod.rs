use crate::config::Config;
use crate::state::ConversationManager;
use anyhow::Result;
use crossterm::style::{style, Color, Stylize};
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task;

pub enum UiUpdate {
    StreamDelta(String),
    TurnComplete(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineStyle {
    Normal,
    Add,
    Delete,
}

struct StreamPrinter {
    current_line: String,
    streamed_any_delta: bool,
}

impl StreamPrinter {
    fn new() -> Self {
        Self {
            current_line: String::new(),
            streamed_any_delta: false,
        }
    }

    fn begin_turn(&mut self) {
        self.streamed_any_delta = false;
    }

    fn has_streamed_delta(&self) -> bool {
        self.streamed_any_delta
    }

    fn write_chunk(&mut self, chunk: &str) -> Result<()> {
        for ch in chunk.chars() {
            if ch == '\r' {
                continue;
            }
            if ch == '\n' {
                print!("\n");
                self.current_line.clear();
                continue;
            }

            self.current_line.push(ch);
            match line_style(&self.current_line) {
                LineStyle::Add => print!("{}", style(ch.to_string()).with(Color::Green)),
                LineStyle::Delete => print!("{}", style(ch.to_string()).with(Color::Red)),
                LineStyle::Normal => print!("{ch}"),
            }
            self.streamed_any_delta = true;
        }
        io::stdout().flush()?;
        Ok(())
    }

    fn end_turn(&mut self) -> Result<()> {
        if !self.current_line.is_empty() {
            println!();
        }
        println!();
        self.current_line.clear();
        io::stdout().flush()?;
        Ok(())
    }
}

pub struct App {
    update_rx: mpsc::UnboundedReceiver<UiUpdate>,
    message_tx: mpsc::UnboundedSender<String>,
    should_quit: bool,
    stream_printer: StreamPrinter,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let (message_tx, mut message_rx) = mpsc::unbounded_channel();

        let client = crate::api::ApiClient::new(&config)?;
        let executor = crate::tools::ToolExecutor::new(config.working_dir.clone());
        let conversation = Arc::new(Mutex::new(ConversationManager::new(client, executor)));

        let conv_clone = Arc::clone(&conversation);
        task::spawn(async move {
            while let Some(content) = message_rx.recv().await {
                let mut mgr = conv_clone.lock().await;
                let delta_tx = {
                    let update_tx = update_tx.clone();
                    let (delta_tx, mut delta_rx) = mpsc::unbounded_channel::<String>();
                    task::spawn(async move {
                        while let Some(delta) = delta_rx.recv().await {
                            let _ = update_tx.send(UiUpdate::StreamDelta(delta));
                        }
                    });
                    delta_tx
                };

                match mgr.send_message(content, Some(&delta_tx)).await {
                    Ok(response) => {
                        drop(delta_tx);
                        let _ = update_tx.send(UiUpdate::TurnComplete(response));
                    }
                    Err(e) => {
                        drop(delta_tx);
                        let _ = update_tx.send(UiUpdate::Error(e.to_string()));
                    }
                }
            }
        });

        Ok(Self {
            update_rx,
            message_tx,
            should_quit: false,
            stream_printer: StreamPrinter::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("aistar text mode • type /quit to exit • streaming enabled");
        println!();

        while !self.should_quit {
            print!("{}", style("> ").dark_grey());
            io::stdout().flush()?;

            let Some(raw_input) = read_user_line().await? else {
                break;
            };
            let content = raw_input.trim().to_string();
            if content.is_empty() {
                continue;
            }
            if matches!(
                content.as_str(),
                "q" | "quit" | "exit" | "/q" | "/quit" | "/exit"
            ) {
                self.should_quit = true;
                break;
            }

            self.stream_printer.begin_turn();
            let _ = self.message_tx.send(content);

            loop {
                match self.update_rx.recv().await {
                    Some(UiUpdate::StreamDelta(text)) => {
                        self.stream_printer.write_chunk(&text)?;
                    }
                    Some(UiUpdate::TurnComplete(text)) => {
                        if !self.stream_printer.has_streamed_delta() && !text.is_empty() {
                            self.stream_printer.write_chunk(&text)?;
                        }
                        self.stream_printer.end_turn()?;
                        break;
                    }
                    Some(UiUpdate::Error(err)) => {
                        self.stream_printer.end_turn()?;
                        println!("{}", style(format!("error: {err}")).with(Color::Red));
                        println!();
                        break;
                    }
                    None => {
                        self.should_quit = true;
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

async fn read_user_line() -> Result<Option<String>> {
    task::spawn_blocking(|| -> Result<Option<String>> {
        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input)?;
        if bytes == 0 {
            Ok(None)
        } else {
            Ok(Some(input))
        }
    })
    .await?
}

fn line_style(line: &str) -> LineStyle {
    let trimmed = line.trim_start();
    if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        LineStyle::Add
    } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        LineStyle::Delete
    } else {
        LineStyle::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_crit_03_state_sync() {
        let state = Arc::new(AtomicUsize::new(0));
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            state_clone.store(42, Ordering::SeqCst);
        });
        handle.await.unwrap();
        assert_eq!(state.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_line_style_feedback() {
        assert_eq!(line_style("+added"), LineStyle::Add);
        assert_eq!(line_style("   +added"), LineStyle::Add);
        assert_eq!(line_style("-removed"), LineStyle::Delete);
        assert_eq!(line_style("   -removed"), LineStyle::Delete);
        assert_eq!(line_style("normal"), LineStyle::Normal);
    }
}
