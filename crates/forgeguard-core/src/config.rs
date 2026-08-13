use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::Severity;

pub const CONFIG_DIR: &str = ".forgeguard";
pub const CONFIG_FILE: &str = ".forgeguard/config.toml";
pub const GLOBAL_CONFIG_FILE: &str = ".forgeguard/config.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GuardMode {
    /// Token-friendly default: report static findings but block only failed required commands.
    #[default]
    Default,
    /// Static report-only mode; required command failures still block.
    Lite,
    /// Full guard mode: blocks failed required commands and version-aware static findings.
    #[serde(alias = "guard")]
    Strict,
}

impl GuardMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Lite => "lite",
            Self::Strict => "strict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePolicy {
    /// Refresh the cache and print a passive notice; never prompts or installs.
    #[default]
    Auto,
    /// Prompt to update on TTY-run commands when a newer version is cached.
    Ask,
    /// Skip the update check entirely.
    Off,
}

impl UpdatePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ask => "ask",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateConfig {
    #[serde(default)]
    pub policy: UpdatePolicy,
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
pub struct FocusConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_focus_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_focus_no_progress_limit")]
    pub no_progress_limit: u32,
    #[serde(default = "default_true")]
    pub auto_poke: bool,
    #[serde(default = "default_focus_max_auto_pokes")]
    pub max_auto_pokes: u32,
    #[serde(default = "default_focus_min_confidence")]
    pub min_confidence: u8,
    #[serde(default = "default_focus_min_hill_climbability")]
    pub min_hill_climbability: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PolicyConfig {
    /// Overrides the mode default. Lite mode always remains report-only for
    /// static findings.
    #[serde(default)]
    pub warnings_block: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RuleConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub severity: Option<Severity>,
    #[serde(default)]
    pub block: Option<bool>,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: default_focus_max_retries(),
            no_progress_limit: default_focus_no_progress_limit(),
            auto_poke: true,
            max_auto_pokes: default_focus_max_auto_pokes(),
            min_confidence: default_focus_min_confidence(),
            min_hill_climbability: default_focus_min_hill_climbability(),
        }
    }
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
    #[serde(default)]
    pub focus: FocusConfig,
    #[serde(default)]
    pub policies: PolicyConfig,
    #[serde(default)]
    pub rules: BTreeMap<String, RuleConfig>,
    #[serde(default)]
    pub update: UpdateConfig,
}

impl ForgeGuardConfig {
    pub fn new(project_name: impl Into<String>, commands: Vec<CommandConfig>) -> Self {
        Self {
            version: default_config_version(),
            mode: GuardMode::Default,
            project: ProjectConfig {
                name: project_name.into(),
            },
            scan: ScanConfig::default(),
            commands,
            focus: FocusConfig::default(),
            policies: PolicyConfig::default(),
            rules: BTreeMap::new(),
            update: UpdateConfig::default(),
        }
    }

    pub fn apply_rule(&self, rule_id: &str, severity: &mut Severity) -> bool {
        let Some(rule) = self.rules.get(rule_id) else {
            return true;
        };
        if rule.enabled == Some(false) {
            return false;
        }
        if let Some(override_severity) = rule.severity {
            *severity = override_severity;
        }
        true
    }

    pub fn blocks_finding(&self, rule_id: &str, severity: Severity) -> bool {
        if self.mode == GuardMode::Lite {
            return false;
        }
        if let Some(block) = self.rules.get(rule_id).and_then(|rule| rule.block) {
            return block;
        }
        match self.policies.warnings_block {
            Some(true) => severity >= Severity::Warning,
            Some(false) => self.mode == GuardMode::Strict && severity == Severity::Error,
            None if self.version >= 2 && self.mode == GuardMode::Strict => {
                severity >= Severity::Warning
            }
            None if self.mode == GuardMode::Strict => severity == Severity::Error,
            None => false,
        }
    }

    pub fn migrate_to_v2(&mut self) -> Result<u32> {
        if !matches!(self.version, 1 | 2) {
            bail!("cannot migrate unsupported config version {}", self.version);
        }
        let previous = self.version;
        self.version = 2;
        Ok(previous)
    }

    pub fn load(root: &Path) -> Result<Self> {
        Self::load_from_path(&root.join(CONFIG_FILE))
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        self.save_to_path(&root.join(CONFIG_FILE))
    }

    pub fn load_global(home: &Path) -> Result<Self> {
        Self::load_from_path(&home.join(GLOBAL_CONFIG_FILE))
    }

    pub fn save_global(&self, home: &Path) -> Result<()> {
        self.save_to_path(&home.join(GLOBAL_CONFIG_FILE))
    }

    fn load_from_path(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = toml::from_str(&source)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if !matches!(config.version, 1 | 2) {
            bail!(
                "unsupported config version {} in {}; expected 1 or 2",
                config.version,
                path.display()
            );
        }
        Ok(config)
    }

    fn save_to_path(&self, path: &Path) -> Result<()> {
        let directory = path
            .parent()
            .context("ForgeGuard config path has no parent")?;
        fs::create_dir_all(directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
        let output = toml::to_string_pretty(self).context("failed to serialize configuration")?;
        fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
    }
}

fn default_true() -> bool {
    true
}

fn default_config_version() -> u32 {
    2
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

fn default_focus_max_retries() -> u32 {
    3
}

fn default_focus_no_progress_limit() -> u32 {
    2
}

fn default_focus_max_auto_pokes() -> u32 {
    3
}

fn default_focus_min_confidence() -> u8 {
    80
}

fn default_focus_min_hill_climbability() -> u8 {
    80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_without_update_section_defaults_to_auto_policy() {
        let toml_source = r#"
version = 2
[project]
name = "demo"
"#;
        let config: ForgeGuardConfig = toml::from_str(toml_source).unwrap();
        assert_eq!(config.update.policy, UpdatePolicy::Auto);
    }

    #[test]
    fn update_policy_round_trips_through_toml() {
        let config = ForgeGuardConfig {
            update: UpdateConfig {
                policy: UpdatePolicy::Ask,
            },
            ..ForgeGuardConfig::new("demo", Vec::new())
        };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: ForgeGuardConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.update.policy, UpdatePolicy::Ask);
    }
}
