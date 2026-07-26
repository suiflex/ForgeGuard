use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const CONFIG_DIR: &str = ".forgeguard";
pub const CONFIG_FILE: &str = ".forgeguard/config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GuardMode {
    Lite,
    #[default]
    Guard,
    Strict,
}

impl GuardMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Guard => "guard",
            Self::Strict => "strict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_file_bytes")]
    pub max_file_bytes: u64,
    #[serde(default)]
    pub include_tests: bool,
    #[serde(default)]
    pub extra_excludes: Vec<String>,
    #[serde(default = "default_duplicate_block_lines")]
    pub duplicate_block_lines: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_file_bytes: default_max_file_bytes(),
            include_tests: false,
            extra_excludes: Vec::new(),
            duplicate_block_lines: default_duplicate_block_lines(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandConfig {
    pub name: String,
    pub command: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_command_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeGuardConfig {
    #[serde(default = "default_config_version")]
    pub version: u32,
    #[serde(default)]
    pub mode: GuardMode,
    pub project: ProjectConfig,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub commands: Vec<CommandConfig>,
}

impl ForgeGuardConfig {
    pub fn new(project_name: impl Into<String>, commands: Vec<CommandConfig>) -> Self {
        Self {
            version: default_config_version(),
            mode: GuardMode::Guard,
            project: ProjectConfig {
                name: project_name.into(),
            },
            scan: ScanConfig::default(),
            commands,
        }
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(CONFIG_FILE);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let directory = root.join(CONFIG_DIR);
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
        let output = toml::to_string_pretty(self).context("failed to serialize configuration")?;
        let path = root.join(CONFIG_FILE);
        fs::write(&path, output).with_context(|| format!("failed to write {}", path.display()))
    }
}

fn default_true() -> bool {
    true
}

fn default_config_version() -> u32 {
    1
}

fn default_max_file_bytes() -> u64 {
    1_000_000
}

fn default_duplicate_block_lines() -> usize {
    6
}

fn default_command_timeout_seconds() -> u64 {
    600
}
