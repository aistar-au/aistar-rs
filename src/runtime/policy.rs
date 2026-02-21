pub trait RuntimeCorePolicy {
    fn sanitize_assistant_text(&self, text: &str) -> String;
    fn request_requires_tool_evidence(&self, input: &str) -> bool;
    fn tool_retry_instruction(&self) -> &'static str;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRuntimeCorePolicy;

const TOOL_RETRY_INSTRUCTION: &str =
    "Your previous answer did not execute any tool call. This request \
requires tool-backed evidence from the workspace. Call the appropriate tool now before \
answering. If structured tool calls are unavailable, use tagged syntax:
<function=tool_name>
<parameter=arg>value</parameter>
</function>";

const TOOL_REQUIRED_HINTS: [&str; 26] = [
    "file",
    "files",
    "directory",
    "directories",
    "tree",
    "repo",
    "repository",
    "cargo.toml",
    "readme",
    "docs/",
    "src/",
    "tests/",
    "version",
    "versions",
    "pinned",
    "count",
    "how many",
    "list",
    "show",
    "search",
    "find",
    "path",
    "line",
    "content of",
    "what's in",
    "whats in",
];

pub fn default_runtime_policy() -> DefaultRuntimeCorePolicy {
    DefaultRuntimeCorePolicy
}

pub fn sanitize_assistant_text(text: &str) -> String {
    default_runtime_policy().sanitize_assistant_text(text)
}

impl RuntimeCorePolicy for DefaultRuntimeCorePolicy {
    fn sanitize_assistant_text(&self, text: &str) -> String {
        strip_tagged_tool_markup(text)
    }

    fn request_requires_tool_evidence(&self, input: &str) -> bool {
        let normalized = input.to_ascii_lowercase();
        TOOL_REQUIRED_HINTS
            .iter()
            .any(|hint| normalized.contains(hint))
    }

    fn tool_retry_instruction(&self) -> &'static str {
        TOOL_RETRY_INSTRUCTION
    }
}

fn strip_tagged_tool_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;

    while let Some(rel_start) = text[cursor..].find("<function=") {
        let start = cursor + rel_start;
        out.push_str(&text[cursor..start]);

        let Some(rel_end) = text[start..].find("</function>") else {
            return strip_incomplete_tool_tag_suffix(&out);
        };
        cursor = start + rel_end + "</function>".len();
    }

    out.push_str(&text[cursor..]);
    strip_incomplete_tool_tag_suffix(&out)
}

fn strip_incomplete_tool_tag_suffix(text: &str) -> String {
    let mut out = text.to_string();
    let Some(last_open) = out.rfind('<') else {
        return out;
    };

    let suffix = &out[last_open..];
    let suffix_lower = suffix.to_ascii_lowercase();
    let looks_like_incomplete_tool_tag = "<function=".starts_with(&suffix_lower)
        || "<function".starts_with(&suffix_lower)
        || "</function>".starts_with(&suffix_lower)
        || "</function".starts_with(&suffix_lower)
        || "<parameter=".starts_with(&suffix_lower)
        || "<parameter".starts_with(&suffix_lower)
        || "</parameter>".starts_with(&suffix_lower)
        || "</parameter".starts_with(&suffix_lower);

    if looks_like_incomplete_tool_tag {
        out.truncate(last_open);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{default_runtime_policy, sanitize_assistant_text, RuntimeCorePolicy};

    #[test]
    fn test_sanitize_assistant_text_removes_tool_block() {
        let text = "Checking.\n<function=git_status>\n</function>\nDone.";
        assert_eq!(sanitize_assistant_text(text), "Checking.\n\nDone.");
    }

    #[test]
    fn test_sanitize_assistant_text_drops_incomplete_tag_suffix() {
        let text = "Checking.\n<function=git_status";
        assert_eq!(sanitize_assistant_text(text), "Checking.\n");
    }

    #[test]
    fn test_request_requires_tool_evidence_detects_repo_facts() {
        let policy = default_runtime_policy();
        assert!(policy.request_requires_tool_evidence("how many files are in this tree"));
        assert!(policy.request_requires_tool_evidence("what's in docs/dev/"));
        assert!(!policy.request_requires_tool_evidence("say hello"));
    }
}
