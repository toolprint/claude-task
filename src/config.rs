use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: String,
    pub paths: PathConfig,
    pub docker: DockerConfig,
    pub claude_user_config: ClaudeUserConfig,
    pub worktree: WorktreeConfig,
    pub global_option_defaults: GlobalOptionDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathConfig {
    pub worktree_base_dir: String,
    pub task_base_home_dir: String,
    pub branch_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerConfig {
    pub image_name: String,
    pub volume_prefix: String,
    pub volumes: DockerVolumes,
    pub container_name_prefix: String,
    pub default_web_view_proxy_port: u16,
    pub default_ht_mcp_port: Option<u16>,
    pub environment_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerVolumes {
    pub home: String,
    pub npm_cache: String,
    pub node_cache: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUserConfig {
    pub config_path: String,
    pub user_memory_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeConfig {
    pub default_open_command: Option<String>,
    pub auto_clean_on_remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalOptionDefaults {
    pub debug: bool,
    pub open_editor_after_create: bool,
    pub build_image_before_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            paths: PathConfig {
                worktree_base_dir: "~/.claude-task/worktrees".to_string(),
                task_base_home_dir: "~/.claude-task/home".to_string(),
                branch_prefix: "claude-task/".to_string(),
            },
            docker: DockerConfig {
                image_name: "claude-task:dev".to_string(),
                volume_prefix: "claude-task-".to_string(),
                volumes: DockerVolumes {
                    home: "claude-task-home".to_string(),
                    npm_cache: "claude-task-npm-cache".to_string(),
                    node_cache: "claude-task-node-cache".to_string(),
                },
                container_name_prefix: "claude-task-".to_string(),
                default_web_view_proxy_port: 4618,
                default_ht_mcp_port: None,
                environment_variables: {
                    let mut env = HashMap::new();
                    env.insert(
                        "CLAUDE_CONFIG_DIR".to_string(),
                        "/home/node/.claude".to_string(),
                    );
                    env
                },
            },
            claude_user_config: ClaudeUserConfig {
                config_path: "~/.claude.json".to_string(),
                user_memory_path: "~/.claude/CLAUDE.md".to_string(),
            },
            worktree: WorktreeConfig {
                default_open_command: None,
                auto_clean_on_remove: false,
            },
            global_option_defaults: GlobalOptionDefaults {
                debug: false,
                open_editor_after_create: false,
                build_image_before_run: false,
            },
        }
    }
}

impl Config {
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".claude-task")
            .join("config.json")
    }

    pub fn load(_path: Option<&PathBuf>) -> Result<Self> {
        // TODO: Implement config loading
        Ok(Self::default())
    }

    pub fn save(&self, _path: &Path) -> Result<()> {
        // TODO: Implement config saving
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        // TODO: Implement validation
        Ok(())
    }

    pub fn expand_tilde(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix("~/") {
            dirs::home_dir()
                .expect("Could not determine home directory")
                .join(stripped)
        } else {
            PathBuf::from(path)
        }
    }
}
