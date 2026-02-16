use crate::config::Config;
use crate::state::{ConversationManager, ConversationStreamUpdate, ToolApprovalRequest};
use anyhow::Result;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task;

const ACTIVITY_MARKER: &str = "*";
const THINKING_MAX_LINES: usize = 4;
const DEFAULT_THINKING_WRAP_WIDTH: usize = 96;

pub enum UiUpdate {
    StreamDelta(String),
    ToolApprovalRequest(ToolApprovalRequest),
    TurnComplete(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolPromptDecision {
    AcceptOnce,
    AcceptSession,
    CancelNewTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineStyle {
    Normal,
    Add,
    Delete,
    Event,
    Thinking,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Normal,
    Thinking,
    Tool,
    Event,
}

struct StreamPrinter {
    current_line: String,
    streamed_any_delta: bool,
    active_style: LineStyle,
    active_block: BlockKind,
    colors_enabled: bool,
    in_code_block: bool,
    code_line_number: usize,
    thinking_rendered_lines: usize,
    thinking_wrap_width: usize,
}

impl StreamPrinter {
    fn new() -> Self {
        Self {
            current_line: String::new(),
            streamed_any_delta: false,
            active_style: LineStyle::Normal,
            active_block: BlockKind::Normal,
            colors_enabled: detect_color_support(),
            in_code_block: false,
            code_line_number: 1,
            thinking_rendered_lines: 0,
            thinking_wrap_width: resolve_thinking_wrap_width(),
        }
    }

    fn begin_turn(&mut self) {
        self.streamed_any_delta = false;
        self.current_line.clear();
        self.active_block = BlockKind::Normal;
        self.in_code_block = false;
        self.code_line_number = 1;
        self.thinking_rendered_lines = 0;
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
                self.finish_current_line()?;
                continue;
            }

            self.current_line.push(ch);
            self.streamed_any_delta = true;
        }
        io::stdout().flush()?;
        Ok(())
    }

    fn end_turn(&mut self) -> Result<()> {
        if !self.current_line.is_empty() {
            self.finish_current_line()?;
        }

        self.set_style(LineStyle::Normal);
        self.current_line.clear();
        io::stdout().flush()?;
        Ok(())
    }

    fn print_code_line_prefix(&mut self) {
        self.set_style(LineStyle::Normal);
        print!(
            "{}",
            format_code_line_prefix(self.code_line_number, self.colors_enabled)
        );
        self.code_line_number += 1;
    }

    fn print_prompt(&mut self) -> Result<()> {
        self.set_style(LineStyle::Normal);
        if self.colors_enabled {
            print!("\x1b[2m> \x1b[0m");
        } else {
            print!("> ");
        }
        io::stdout().flush()?;
        Ok(())
    }

    fn print_error(&mut self, message: &str) -> Result<()> {
        self.set_style(LineStyle::Normal);
        if self.colors_enabled {
            println!("\x1b[31merror: {message}\x1b[0m");
        } else {
            println!("error: {message}");
        }
        println!();
        io::stdout().flush()?;
        Ok(())
    }

    fn ensure_newline(&mut self) -> Result<()> {
        self.set_style(LineStyle::Normal);
        if !self.current_line.is_empty() {
            println!();
            self.current_line.clear();
        }
        io::stdout().flush()?;
        Ok(())
    }

    fn print_tool_approval_prompt(&mut self, name: &str, input_preview: &str) -> Result<()> {
        self.ensure_newline()?;
        self.set_style(LineStyle::Tool);
        println!("{ACTIVITY_MARKER} Tool Execution: {name}");

        for (idx, line) in input_preview.lines().enumerate() {
            let prefix = if idx == 0 {
                "  └ "
            } else if is_numbered_preview_line(line) {
                ""
            } else {
                "    "
            };
            let style = match line_style(line, false, self.colors_enabled) {
                LineStyle::Add => LineStyle::Add,
                LineStyle::Delete => LineStyle::Delete,
                _ => LineStyle::Event,
            };
            self.set_style(style);
            println!("{prefix}{line}");
        }

        self.set_style(LineStyle::Event);
        println!("* Prompt");
        println!("  │ 1 accept and continue");
        println!("  │ 2 accept and continue (session)");
        println!("  └ 3 cancel and start new task");

        self.set_style(LineStyle::Normal);
        if self.colors_enabled {
            print!("\x1b[1m* Select > \x1b[0m");
        } else {
            print!("* Select > ");
        }
        io::stdout().flush()?;
        Ok(())
    }

    fn print_session_auto_approve_notice(&mut self) -> Result<()> {
        self.ensure_newline()?;
        self.set_style(LineStyle::Event);
        println!("* Prompt");
        println!("  └ session auto-approve enabled");
        self.set_style(LineStyle::Normal);
        io::stdout().flush()?;
        Ok(())
    }

    fn render_line(&mut self, line: &str) -> Result<bool> {
        if self.in_code_block {
            if line.trim_start().starts_with("```") {
                self.set_style(LineStyle::Normal);
                print!("{line}");
                return Ok(true);
            }

            self.print_code_line_prefix();
            let style = line_style(line, true, self.colors_enabled);
            self.set_style(style);
            print!("{line}");
            return Ok(true);
        }

        let inline_thinking_text = thinking_inline_text(line);
        if self.active_block == BlockKind::Thinking
            && (!looks_like_activity_line(line) || inline_thinking_text.is_some())
            && !line.trim_start().starts_with("```")
        {
            let source_text = inline_thinking_text.unwrap_or_else(|| line.trim_start().to_string());
            let wrapped = wrap_text_for_display(&source_text, self.thinking_wrap_width);
            let mut out_lines = Vec::new();
            for segment in wrapped {
                let blob_line_index = self.thinking_rendered_lines % THINKING_MAX_LINES;
                let prefix = if is_checklist_like(segment.as_str()) {
                    "    "
                } else {
                    thinking_prefix(blob_line_index)
                };
                out_lines.push(format!("{prefix}{segment}"));
                self.thinking_rendered_lines += 1;
            }

            if out_lines.is_empty() {
                return Ok(false);
            }

            self.set_style(LineStyle::Thinking);
            print!("{}", out_lines.join("\n"));
            return Ok(true);
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("* Event:") || trimmed.starts_with("* Tool:") {
            return Ok(false);
        }

        let output =
            normalize_existing_numbered_snippet_line(line).unwrap_or_else(|| line.to_string());
        if output.is_empty() {
            return Ok(false);
        }

        let style = line_style(&output, false, self.colors_enabled);
        self.set_style(style);
        print!("{output}");
        Ok(true)
    }

    fn set_style(&mut self, style: LineStyle) {
        if !self.colors_enabled || self.active_style == style {
            self.active_style = style;
            return;
        }

        if self.active_style != LineStyle::Normal {
            print!("\x1b[0m");
        }

        match style {
            LineStyle::Add => print!("\x1b[1;32m"),
            LineStyle::Delete => print!("\x1b[1;31m"),
            LineStyle::Event => print!("\x1b[2m"),
            LineStyle::Thinking => print!("\x1b[2;90m"),
            LineStyle::Tool => print!("\x1b[33m"),
            LineStyle::Normal => {}
        }
        self.active_style = style;
    }

    fn update_code_block_state_for_finished_line(&mut self, line: &str) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            self.in_code_block = !self.in_code_block;
            if self.in_code_block {
                self.code_line_number = 1;
                self.thinking_rendered_lines = 0;
            }
        }
    }

    fn update_block_context_for_finished_line(&mut self, line: &str) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("* Thinking") {
            self.active_block = BlockKind::Thinking;
            self.thinking_rendered_lines = 0;
        } else if self.active_block == BlockKind::Thinking && thinking_inline_text(line).is_some() {
            // Keep tool-call markers folded inside the active thinking block.
        } else if trimmed.starts_with("* Tool") {
            self.active_block = BlockKind::Tool;
            self.thinking_rendered_lines = 0;
        } else if trimmed.starts_with("* Event: message_stop") {
            self.active_block = BlockKind::Normal;
            self.thinking_rendered_lines = 0;
        } else if trimmed.starts_with("* Event:") {
            self.active_block = BlockKind::Event;
            self.thinking_rendered_lines = 0;
        } else if self.active_block == BlockKind::Thinking
            && trimmed.is_empty()
            && !self.in_code_block
        {
            self.thinking_rendered_lines = 0;
        }
    }

    fn finish_current_line(&mut self) -> Result<()> {
        let line = std::mem::take(&mut self.current_line);
        let rendered = self.render_line(&line)?;
        if rendered {
            println!();
        }
        self.update_code_block_state_for_finished_line(&line);
        self.update_block_context_for_finished_line(&line);
        Ok(())
    }
}

pub struct App {
    update_rx: mpsc::UnboundedReceiver<UiUpdate>,
    message_tx: mpsc::UnboundedSender<String>,
    should_quit: bool,
    auto_approve_tools: bool,
    suppress_until_turn_complete: bool,
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
                    let (delta_tx, mut delta_rx) =
                        mpsc::unbounded_channel::<ConversationStreamUpdate>();
                    task::spawn(async move {
                        while let Some(delta) = delta_rx.recv().await {
                            let ui_update = match delta {
                                ConversationStreamUpdate::Delta(text) => {
                                    UiUpdate::StreamDelta(text)
                                }
                                ConversationStreamUpdate::ToolApprovalRequest(request) => {
                                    UiUpdate::ToolApprovalRequest(request)
                                }
                            };
                            let _ = update_tx.send(ui_update);
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
            auto_approve_tools: false,
            suppress_until_turn_complete: false,
            stream_printer: StreamPrinter::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        while !self.should_quit {
            self.stream_printer.print_prompt()?;

            let Some(raw_input) = read_user_line().await? else {
                break;
            };
            let content = raw_input.trim().to_string();
            if content.is_empty() {
                continue;
            }
            if is_escape_command(content.as_str()) {
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
                        if self.suppress_until_turn_complete {
                            continue;
                        }
                        self.stream_printer.write_chunk(&text)?;
                    }
                    Some(UiUpdate::ToolApprovalRequest(request)) => {
                        if self.auto_approve_tools {
                            let _ = request.response_tx.send(true);
                            continue;
                        }

                        self.stream_printer.print_tool_approval_prompt(
                            &request.tool_name,
                            &request.input_preview,
                        )?;
                        let decision = read_tool_confirmation().await?;
                        match decision {
                            ToolPromptDecision::AcceptOnce => {
                                let _ = request.response_tx.send(true);
                            }
                            ToolPromptDecision::AcceptSession => {
                                self.auto_approve_tools = true;
                                self.stream_printer.print_session_auto_approve_notice()?;
                                let _ = request.response_tx.send(true);
                            }
                            ToolPromptDecision::CancelNewTask => {
                                self.suppress_until_turn_complete = true;
                                let _ = request.response_tx.send(false);
                            }
                        }
                        self.stream_printer.set_style(LineStyle::Normal);
                        println!();
                    }
                    Some(UiUpdate::TurnComplete(text)) => {
                        if self.suppress_until_turn_complete {
                            self.suppress_until_turn_complete = false;
                            self.stream_printer.end_turn()?;
                            break;
                        }
                        if !self.stream_printer.has_streamed_delta() && !text.is_empty() {
                            self.stream_printer.write_chunk(&text)?;
                        }
                        self.stream_printer.end_turn()?;
                        break;
                    }
                    Some(UiUpdate::Error(err)) => {
                        if self.suppress_until_turn_complete {
                            self.suppress_until_turn_complete = false;
                            self.stream_printer.end_turn()?;
                            break;
                        }
                        self.stream_printer.end_turn()?;
                        self.stream_printer.print_error(&err)?;
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

async fn read_tool_confirmation() -> Result<ToolPromptDecision> {
    loop {
        let Some(raw) = read_user_line().await? else {
            return Ok(ToolPromptDecision::CancelNewTask);
        };
        let trimmed = raw.trim();
        if is_escape_command(trimmed) {
            return Ok(ToolPromptDecision::CancelNewTask);
        }
        if trimmed.starts_with('1') {
            return Ok(ToolPromptDecision::AcceptOnce);
        }
        if trimmed.starts_with('2') {
            return Ok(ToolPromptDecision::AcceptSession);
        }
        if trimmed.starts_with('3') {
            return Ok(ToolPromptDecision::CancelNewTask);
        }
        println!("* Prompt");
        println!("  └ enter 1, 2, 3, or esc");
        print!("* Select > ");
        io::stdout().flush()?;
    }
}

fn line_style(line: &str, in_code_block: bool, colors_enabled: bool) -> LineStyle {
    if !colors_enabled {
        return LineStyle::Normal;
    }

    let trimmed = strip_optional_number_prefix(line);
    if in_code_block && trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        LineStyle::Add
    } else if in_code_block && trimmed.starts_with('-') && !trimmed.starts_with("---") {
        LineStyle::Delete
    } else if has_number_prefix(line) && trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        LineStyle::Add
    } else if has_number_prefix(line) && trimmed.starts_with('-') && !trimmed.starts_with("---") {
        LineStyle::Delete
    } else if trimmed.starts_with("+ [tool_result]") {
        LineStyle::Add
    } else if trimmed.starts_with("- [tool_error]") {
        LineStyle::Delete
    } else if trimmed.starts_with("* Thinking") {
        LineStyle::Thinking
    } else if trimmed.starts_with("* Tool") {
        LineStyle::Tool
    } else if trimmed.starts_with("* Event:") {
        LineStyle::Event
    } else {
        LineStyle::Normal
    }
}

fn strip_optional_number_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some((left, right)) = trimmed.split_once('|') {
        let left = left.trim();
        if !left.is_empty() && left.chars().all(|c| c.is_ascii_digit()) {
            return right.trim_start();
        }
    }

    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &trimmed[digits..];
        if rest.starts_with(' ') {
            return rest.trim_start();
        }
    }
    trimmed
}

fn has_number_prefix(line: &str) -> bool {
    let trimmed = line.trim_start();
    if let Some((left, _)) = trimmed.split_once('|') {
        let left = left.trim();
        if !left.is_empty() && left.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }

    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    trimmed[digits..].starts_with(' ')
}

fn format_code_line_prefix(line_number: usize, colors_enabled: bool) -> String {
    if colors_enabled {
        format!("  \x1b[1m{line_number}\x1b[0m ")
    } else {
        format!("  {line_number} ")
    }
}

fn normalize_existing_numbered_snippet_line(line: &str) -> Option<String> {
    if looks_like_activity_line(line) {
        return None;
    }

    let trimmed = line.trim_start();
    let leading_spaces = line.chars().take_while(|c| c.is_ascii_whitespace()).count();

    if let Some((left, right)) = trimmed.split_once('|') {
        let number = left.trim();
        if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("  {number} {}", right.trim_start()));
        }
    }

    let digit_count = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digit_count == 0 || leading_spaces < 2 {
        return None;
    }

    let number = trimmed[..digit_count].trim();
    let rest = trimmed[digit_count..].trim_start();
    if rest.is_empty() {
        return None;
    }

    Some(format!("  {number} {rest}"))
}

fn detect_color_support() -> bool {
    if std::env::var("AISTAR_FORCE_COLOR")
        .ok()
        .and_then(parse_bool_flag)
        .unwrap_or(false)
    {
        return true;
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    io::stdout().is_terminal()
}

fn parse_bool_flag(value: String) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn looks_like_activity_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("* Thinking")
        || trimmed.starts_with("* Tool")
        || trimmed.starts_with("* Event:")
        || trimmed.starts_with("* Tool Execution:")
}

fn is_escape_command(input: &str) -> bool {
    input == "\u{1b}" || matches!(input, "esc" | "/esc" | "escape" | "/escape")
}

fn thinking_inline_text(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if let Some(tool) = trimmed.strip_prefix("* Tool:") {
        return Some(format!("Tool call:{}.", tool.trim()));
    }
    if let Some(event) = trimmed.strip_prefix("* Event: input_json#") {
        return Some(format!("Tool input stream: input_json#{}.", event.trim()));
    }
    if trimmed.starts_with("* Event: stop_reason=tool_use") {
        return Some("Assistant paused for tool execution.".to_string());
    }
    None
}

fn thinking_prefix(line_index: usize) -> &'static str {
    if line_index + 1 >= THINKING_MAX_LINES {
        "  └ "
    } else {
        "  │ "
    }
}

fn wrap_text_for_display(text: &str, width: usize) -> Vec<String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut current = String::new();

    for word in clean.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn is_numbered_preview_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && trimmed[digits..].starts_with(' ') {
        return true;
    }

    line.starts_with("  ...")
}

fn is_checklist_like(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("- ") || trimmed.starts_with("• ") || trimmed.starts_with("* ") {
        return true;
    }

    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0
        && (trimmed[digits..].starts_with(". ")
            || trimmed[digits..].starts_with(") ")
            || trimmed[digits..].starts_with(" - "))
}

fn resolve_thinking_wrap_width() -> usize {
    std::env::var("AISTAR_THINKING_WRAP_WIDTH")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(40, 160))
        .unwrap_or(DEFAULT_THINKING_WRAP_WIDTH)
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
        assert_eq!(line_style("+added", true, true), LineStyle::Add);
        assert_eq!(line_style("   +added", true, true), LineStyle::Add);
        assert_eq!(line_style("  12 | +added", true, true), LineStyle::Add);
        assert_eq!(line_style("-removed", true, true), LineStyle::Delete);
        assert_eq!(line_style("   -removed", true, true), LineStyle::Delete);
        assert_eq!(line_style("9| -removed", true, true), LineStyle::Delete);
        assert_eq!(line_style("    12 +added", false, true), LineStyle::Add);
        assert_eq!(
            line_style("    12 -removed", false, true),
            LineStyle::Delete
        );
        assert_eq!(
            line_style("+ [tool_result] read_file", false, true),
            LineStyle::Add
        );
        assert_eq!(
            line_style("- [tool_error] edit_file: failed", false, true),
            LineStyle::Delete
        );
        assert_eq!(line_style("* Thinking", false, true), LineStyle::Thinking);
        assert_eq!(
            line_style("* Tool: edit_file", false, true),
            LineStyle::Tool
        );
        assert_eq!(
            line_style("* Event: message_start", false, true),
            LineStyle::Event
        );
        assert_eq!(line_style("normal", false, true), LineStyle::Normal);
        assert_eq!(line_style("+added", true, false), LineStyle::Normal);
    }

    #[test]
    fn test_code_block_toggle() {
        let mut printer = StreamPrinter::new();
        printer.update_code_block_state_for_finished_line("```rust");
        assert!(printer.in_code_block);
        assert_eq!(printer.code_line_number, 1);

        printer.update_code_block_state_for_finished_line("```");
        assert!(!printer.in_code_block);
    }

    #[test]
    fn test_format_code_line_prefix_alignment() {
        assert_eq!(format_code_line_prefix(1, false), "  1 ");
        assert_eq!(format_code_line_prefix(604, false), "  604 ");
    }

    #[test]
    fn test_normalize_existing_numbered_snippet_line() {
        assert_eq!(
            normalize_existing_numbered_snippet_line("   12 | +hello").as_deref(),
            Some("  12 +hello")
        );
        assert_eq!(
            normalize_existing_numbered_snippet_line("604 | println!(\"ok\");").as_deref(),
            Some("  604 println!(\"ok\");")
        );
        assert_eq!(
            normalize_existing_numbered_snippet_line("    603 +        assert_eq!(x, y);")
                .as_deref(),
            Some("  603 +        assert_eq!(x, y);")
        );
        assert!(normalize_existing_numbered_snippet_line("* Event: message_start").is_none());
        assert!(normalize_existing_numbered_snippet_line("not a numbered line").is_none());
    }

    #[test]
    fn test_activity_and_escape_helpers() {
        assert!(looks_like_activity_line("* Thinking"));
        assert!(looks_like_activity_line("* Tool: read_file"));
        assert!(looks_like_activity_line("* Event: message_stop"));
        assert!(!looks_like_activity_line("normal line"));

        assert!(is_escape_command("\u{1b}"));
        assert!(is_escape_command("esc"));
        assert!(is_escape_command("/escape"));
        assert!(!is_escape_command("1"));
    }

    #[test]
    fn test_thinking_prefix_shape() {
        assert_eq!(thinking_prefix(0), "  │ ");
        assert_eq!(thinking_prefix(1), "  │ ");
        assert_eq!(thinking_prefix(2), "  │ ");
        assert_eq!(thinking_prefix(3), "  └ ");
    }

    #[test]
    fn test_wrap_text_for_display() {
        let wrapped = wrap_text_for_display(
            "this is a sentence that should wrap into multiple lines",
            16,
        );
        assert_eq!(
            wrapped,
            vec![
                "this is a".to_string(),
                "sentence that".to_string(),
                "should wrap into".to_string(),
                "multiple lines".to_string()
            ]
        );
    }

    #[test]
    fn test_is_numbered_preview_line() {
        assert!(is_numbered_preview_line("  12 - line"));
        assert!(is_numbered_preview_line("  ... (4 more lines)"));
        assert!(!is_numbered_preview_line("old_str: 12 chars"));
    }

    #[test]
    fn test_thinking_inline_text() {
        assert_eq!(
            thinking_inline_text("* Tool: read_file").as_deref(),
            Some("Tool call:read_file.")
        );
        assert_eq!(
            thinking_inline_text("* Event: input_json#1").as_deref(),
            Some("Tool input stream: input_json#1.")
        );
        assert_eq!(
            thinking_inline_text("* Event: stop_reason=tool_use").as_deref(),
            Some("Assistant paused for tool execution.")
        );
        assert!(thinking_inline_text("* Event: message_start").is_none());
    }

    #[test]
    fn test_is_checklist_like() {
        assert!(is_checklist_like("- task"));
        assert!(is_checklist_like("1. task"));
        assert!(is_checklist_like("• task"));
        assert!(!is_checklist_like("plain sentence"));
    }
}
