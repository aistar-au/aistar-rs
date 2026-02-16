use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub working_dir: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY not set")?;

        Ok(Self {
            api_key,
            model: "claude-sonnet-4-5-20250929".to_string(),
            working_dir: std::env::current_dir()?,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.model.starts_with("local/") {
            bail!("Local models not supported in v0.1.0");
        }

        if !self.model.starts_with("claude-") {
            bail!(
                "Invalid model name: '{}'. Expected a model starting with 'claude-'",
                self.model
            );
        }

        Ok(())
    }
}
