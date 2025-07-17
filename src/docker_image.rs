use std::env;
use std::process::Command;

/// Get the default Docker image name based on GitHub organization
pub fn get_default_docker_image() -> String {
    // Allow environment variable override
    if let Ok(org) = env::var("CLAUDE_TASK_DOCKER_ORG") {
        return format!("ghcr.io/{}/claude-task:latest", org);
    }
    
    // Try to get the repository owner using gh CLI
    let output = Command::new("gh")
        .arg("repo")
        .arg("view")
        .arg("--json")
        .arg("owner")
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            
            // Simple JSON parsing for owner.login
            // Expected format: {"owner":{"id":"...","login":"toolprint"}}
            if let Some(start) = json_str.find(r#""login":"#) {
                let start = start + 9; // length of "login":"
                if let Some(end) = json_str[start..].find('"') {
                    let org = json_str[start..start + end].to_lowercase();
                    return format!("ghcr.io/{}/claude-task:latest", org);
                }
            }
        }
    }
    
    // Default fallback
    "ghcr.io/toolprint/claude-task:latest".to_string()
}

/// Lazy static to cache the Docker image name
use std::sync::OnceLock;

static DEFAULT_DOCKER_IMAGE: OnceLock<String> = OnceLock::new();

/// Get the default Docker image name (cached)
pub fn default_docker_image() -> &'static str {
    DEFAULT_DOCKER_IMAGE.get_or_init(get_default_docker_image)
}