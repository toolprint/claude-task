use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use rand::Rng;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const ADJECTIVES: &[&str] = &[
    "autumn",
    "hidden",
    "bitter",
    "misty",
    "silent",
    "empty",
    "dry",
    "dark",
    "summer",
    "icy",
    "delicate",
    "quiet",
    "white",
    "cool",
    "spring",
    "winter",
    "patient",
    "twilight",
    "dawn",
    "crimson",
    "wispy",
    "weathered",
    "blue",
    "billowing",
    "broken",
    "cold",
    "damp",
    "falling",
    "frosty",
    "green",
    "long",
    "late",
    "lingering",
    "bold",
    "little",
    "morning",
    "muddy",
    "old",
    "red",
    "rough",
    "still",
    "small",
    "sparkling",
    "throbbing",
    "shy",
    "wandering",
    "withered",
    "wild",
    "black",
    "young",
    "holy",
    "solitary",
    "fragrant",
    "aged",
    "snowy",
    "proud",
    "floral",
    "restless",
    "divine",
    "polished",
    "ancient",
    "purple",
    "lively",
    "nameless",
];

const NOUNS: &[&str] = &[
    "waterfall",
    "river",
    "breeze",
    "moon",
    "rain",
    "wind",
    "sea",
    "morning",
    "snow",
    "lake",
    "sunset",
    "pine",
    "shadow",
    "leaf",
    "dawn",
    "glade",
    "forest",
    "hill",
    "cloud",
    "meadow",
    "sun",
    "glitter",
    "brook",
    "butterfly",
    "bush",
    "dew",
    "dust",
    "field",
    "fire",
    "flower",
    "firefly",
    "feather",
    "grass",
    "haze",
    "mountain",
    "night",
    "pond",
    "darkness",
    "snowflake",
    "silence",
    "sound",
    "sky",
    "shape",
    "surf",
    "thunder",
    "violet",
    "water",
    "wildflower",
    "wave",
    "water",
    "resonance",
    "sun",
    "wood",
    "dream",
    "cherry",
    "tree",
    "fog",
    "frost",
    "voice",
    "paper",
    "frog",
    "smoke",
    "star",
];

pub fn generate_short_id() -> String {
    let mut rng = rand::rng();
    let adjective = ADJECTIVES[rng.random_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.random_range(0..NOUNS.len())];
    let number = rng.random_range(1000..10000);
    format!("{adjective}-{noun}-{number}")
}

pub fn open_worktree(worktree_path: &str, open_command: Option<&str>) -> Result<()> {
    let command = open_command.unwrap_or("code"); // Default to VS Code
    println!("Attempting to open worktree in editor with command: `{command} {worktree_path}`");

    let status = Command::new(command)
        .arg(worktree_path)
        .status()
        .with_context(|| format!("Failed to execute open command: '{command}'"))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Open command failed with status: {}",
            status
        ));
    }

    println!("✓ Successfully opened worktree in editor.");
    Ok(())
}

pub fn select_worktree_interactively(
    branch_prefix: &str,
    open_command: Option<&str>,
) -> Result<()> {
    let worktrees = get_matching_worktrees(branch_prefix)?;

    if worktrees.is_empty() {
        println!("No worktrees found to open.");
        return Ok(());
    }

    let selections: Vec<String> = worktrees
        .iter()
        .map(|(path, _, branch)| {
            let clean_branch = if branch.starts_with("refs/heads/") {
                branch.strip_prefix("refs/heads/").unwrap_or(branch)
            } else {
                branch
            };
            format!("{clean_branch} ({path})")
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a worktree to open")
        .default(0)
        .items(&selections)
        .interact_opt()?
        .map(|index| &worktrees[index].0);

    if let Some(worktree_path) = selection {
        open_worktree(worktree_path, open_command)?;
    } else {
        println!("No worktree selected.");
    }

    Ok(())
}

pub async fn clean_all_worktrees(
    branch_prefix: &str,
    skip_confirmation: bool,
    force: bool,
    auto_clean_branch: bool,
) -> Result<()> {
    let worktrees = get_matching_worktrees(branch_prefix)?;

    if worktrees.is_empty() {
        println!("No worktrees to clean up.");
        return Ok(());
    }

    println!("The following worktrees will be removed:");
    for (path, _, branch) in &worktrees {
        println!("- {branch} ({path})");
    }

    if !skip_confirmation {
        print!("Are you sure you want to remove all these worktrees? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;
        if input.trim().to_lowercase() != "y" {
            println!("Cleanup cancelled.");
            return Ok(());
        }
    }

    for (path, _, branch) in worktrees {
        let clean_branch = if branch.starts_with("refs/heads/") {
            branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch.as_str())
        } else {
            branch.as_str()
        };

        let task_id = clean_branch
            .strip_prefix(branch_prefix)
            .unwrap_or(clean_branch);

        if !force {
            let status = check_worktree_status(&PathBuf::from(&path))?;
            if !status.is_clean() {
                println!("Skipping unclean worktree: {path} (use --force to remove)");
                continue;
            }
        }
        remove_git_worktree(task_id, branch_prefix, auto_clean_branch)?;
    }

    Ok(())
}

pub fn sanitize_branch_name(name: &str) -> String {
    let re = Regex::new(r"[^a-zA-Z0-9\-_]").unwrap();
    re.replace_all(name, "-").to_string()
}

pub fn find_git_repo_root(start_path: &Path) -> Result<PathBuf> {
    let mut current = start_path;
    loop {
        if current.join(".git").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(anyhow::anyhow!("No git repository found")),
        }
    }
}

pub fn get_repo_name(worktree_path: &Path) -> String {
    // Try to get repo name from git remote
    // Git handles worktrees automatically, so we can just run from the worktree path
    if let Ok(output) = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Extract org/repo from URL
            // Handle HTTPS format: https://github.com/org/repo.git
            if url.starts_with("https://") || url.starts_with("http://") {
                // Remove protocol and domain
                let path_part = url
                    .split("://")
                    .nth(1)
                    .and_then(|s| s.split('/').skip(1).collect::<Vec<_>>().join("/").into());

                if let Some(path) = path_part {
                    let clean_path = path.strip_suffix(".git").unwrap_or(&path);
                    if clean_path.contains('/') {
                        return clean_path.to_string();
                    }
                }
            }

            // Handle SSH format: git@github.com:org/repo.git
            if let Some(repo_part) = url.split(':').nth(1) {
                let clean_path = repo_part.strip_suffix(".git").unwrap_or(repo_part);
                if clean_path.contains('/') {
                    return clean_path.to_string();
                }
            }
        }
    }

    // Fallback to directory name
    worktree_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn get_worktree_directory(worktree_base_dir: &str) -> Result<PathBuf> {
    let worktree_dir = worktree_base_dir.to_string();

    let worktree_path = if worktree_dir.starts_with('/') || worktree_dir.starts_with('~') {
        // Absolute path or home directory path
        if worktree_dir.starts_with('~') {
            let home_dir = std::env::var("HOME").context("Could not find HOME directory")?;
            PathBuf::from(worktree_dir.replacen('~', &home_dir, 1))
        } else {
            PathBuf::from(worktree_dir)
        }
    } else {
        // Relative path - relative to current directory
        std::env::current_dir()
            .context("Could not get current directory")?
            .join(worktree_dir)
    };

    fs::create_dir_all(&worktree_path)
        .with_context(|| format!("Failed to create worktree directory: {worktree_path:?}"))?;
    Ok(worktree_path)
}

pub fn create_git_worktree(
    task_id: &str,
    branch_prefix: &str,
    worktree_base_dir: &str,
) -> Result<(PathBuf, String)> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let sanitized_name = sanitize_branch_name(task_id);
    let branch_name = format!("{branch_prefix}{sanitized_name}");

    let worktree_base_dir = get_worktree_directory(worktree_base_dir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let worktree_path = worktree_base_dir.join(format!("{sanitized_name}_{timestamp:x}"));

    println!("Creating git worktree...");
    println!("Repository root: {repo_root:?}");
    println!("Branch name: {branch_name}");
    println!("Worktree path: {worktree_path:?}");

    // Create the worktree
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Git worktree command failed: {}", stderr));
    }

    println!("✓ Git worktree created successfully");
    println!("  Branch: {branch_name}");
    println!("  Path: {worktree_path:?}");

    Ok((worktree_path, branch_name))
}

pub fn list_git_worktrees(branch_prefix: &str) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    println!("Listing git worktrees with branch prefix '{branch_prefix}'...");
    println!("Repository root: {repo_root:?}");
    println!();

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        println!("No worktrees found.");
        return Ok(());
    }

    let mut current_worktree: Option<(String, String, String)> = None; // (path, head, branch)
    let mut matching_worktrees = Vec::new();

    for line in lines {
        if line.starts_with("worktree ") {
            // If we have a previous worktree, check if it matches and store it
            if let Some((path, head, branch)) = current_worktree.take() {
                if should_include_worktree(&branch, branch_prefix, &path, &repo_root) {
                    matching_worktrees.push((path, head, branch));
                }
            }

            // Start new worktree
            let path = line.strip_prefix("worktree ").unwrap_or(line);
            current_worktree = Some((path.to_string(), String::new(), String::new()));
        } else if line.starts_with("HEAD ") {
            if let Some((_, ref mut head, _)) = current_worktree.as_mut() {
                let new_head = line.strip_prefix("HEAD ").unwrap_or(line);
                *head = new_head.to_string();
            }
        } else if line.starts_with("branch ") {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                let new_branch = line.strip_prefix("branch ").unwrap_or(line);
                *branch = new_branch.to_string();
            }
        } else if line == "bare" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(bare)".to_string();
            }
        } else if line == "detached" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(detached)".to_string();
            }
        }
    }

    // Handle the last worktree if it exists
    if let Some((path, head, branch)) = current_worktree {
        if should_include_worktree(&branch, branch_prefix, &path, &repo_root) {
            matching_worktrees.push((path, head, branch));
        }
    }

    // Print all matching worktrees
    if matching_worktrees.is_empty() {
        println!("No worktrees found matching branch prefix '{branch_prefix}'.");
    } else {
        for (path, head, branch) in matching_worktrees {
            print_worktree_info(&path, &head, &branch);
        }
    }

    Ok(())
}

pub fn should_include_worktree(
    branch: &str,
    branch_prefix: &str,
    path: &str,
    repo_root: &std::path::Path,
) -> bool {
    // Clean up branch name by removing refs/heads/ prefix for comparison
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else {
        branch
    };

    // Exclude the main repository directory (where .git folder is located)
    let worktree_path = std::path::Path::new(path);
    if worktree_path == repo_root {
        return false;
    }

    // Only include branches that start with the prefix (exclude main/master unless they're actual worktrees)
    clean_branch.starts_with(branch_prefix)
        || branch == "(bare)"
        || branch == "(detached)"
        // Include main/master only if they are actual worktrees (not the main repo)
        || ((clean_branch == "main" || clean_branch == "master") && worktree_path != repo_root)
}

pub fn print_worktree_info(path: &str, head: &str, branch: &str) {
    let path_buf = PathBuf::from(path);
    let dir_name = path_buf
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown");

    // Clean up branch name by removing refs/heads/ prefix
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else if branch.is_empty() {
        "unknown"
    } else {
        branch
    };

    // Determine if this is a Claude task worktree
    let is_claude_task = clean_branch.starts_with("claude-task/");
    let icon = if is_claude_task { "🌿" } else { "📁" };
    let type_label = if is_claude_task {
        " (Claude task)"
    } else {
        " (worktree)"
    };

    // Get repository name
    let repo_name = get_repo_name(&path_buf);

    // Check worktree status
    println!("{icon} {dir_name}{type_label}");
    println!("   Path: {path}");
    println!("   Repository: {repo_name}");
    println!("   Branch: {clean_branch}");
    println!(
        "   HEAD: {}",
        if head.len() > 7 { &head[..7] } else { head }
    );

    match check_worktree_status(&path_buf) {
        Ok(status) => {
            let status_icon = status.get_status_icon();
            let details = status.get_status_details();

            if status.is_clean() {
                if status.is_likely_merged {
                    let merge_type = status.merge_info.as_deref().unwrap_or("merged");
                    println!("   Status: {status_icon} Clean ({merge_type})");
                } else {
                    println!("   Status: {status_icon} Clean");
                }
            } else {
                println!("   Status: {status_icon} Unclean: {}", details.join(", "));

                // Show merge info if detected
                if status.is_likely_merged {
                    if let Some(ref info) = status.merge_info {
                        println!(
                            "   Note: Branch appears to be {info} - remote may have been deleted"
                        );
                    }
                }

                // Show changed files if any
                if !status.changed_files.is_empty() {
                    println!("   Changed files:");
                    for file in &status.changed_files {
                        println!("     - {file}");
                    }
                }

                // Show untracked files if any
                if !status.untracked_files.is_empty() {
                    println!("   Untracked files:");
                    for file in &status.untracked_files {
                        println!("     - {file}");
                    }
                }

                // Show unpushed commits if any
                if !status.unpushed_commits.is_empty() && !status.is_likely_merged {
                    println!("   Unpushed commits:");
                    for (commit_id, message) in &status.unpushed_commits {
                        println!("     - {commit_id} {message}");
                    }
                } else if !status.unpushed_commits.is_empty() && status.is_likely_merged {
                    println!("   Commits (likely already merged):");
                    for (commit_id, message) in &status.unpushed_commits {
                        println!("     - {commit_id} {message}");
                    }
                }
            }
        }
        Err(_) => {
            println!("   Status: ❓ Status unknown");
        }
    };

    println!();
}

pub fn remove_git_worktree(
    task_id: &str,
    branch_prefix: &str,
    auto_clean_branch: bool,
) -> Result<()> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let sanitized_id = sanitize_branch_name(task_id);
    let branch_name = format!("{branch_prefix}{sanitized_id}");

    println!("Removing git worktree for task '{task_id}'...");
    println!("Repository root: {repo_root:?}");
    println!("Target branch: {branch_name}");
    println!();

    // First, get list of worktrees to find the one with matching branch
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    let mut worktree_path: Option<String> = None;
    let mut current_path: Option<String> = None;

    for line in lines {
        if line.starts_with("worktree ") {
            current_path = Some(line.strip_prefix("worktree ").unwrap_or(line).to_string());
        } else if line.starts_with("branch ") {
            let branch = line.strip_prefix("branch ").unwrap_or(line);
            let clean_branch = if branch.starts_with("refs/heads/") {
                branch.strip_prefix("refs/heads/").unwrap_or(branch)
            } else {
                branch
            };

            if clean_branch == branch_name {
                worktree_path = current_path.clone();
                break;
            }
        }
    }

    let worktree_path = match worktree_path {
        Some(path) => path,
        None => {
            println!("❌ No worktree found for branch '{branch_name}'");
            return Ok(());
        }
    };

    println!("Found worktree: {worktree_path}");

    // Remove the worktree
    println!("Removing worktree...");
    let output = Command::new("git")
        .args(["worktree", "remove", &worktree_path, "--force"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree remove command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree remove command failed: {}",
            stderr
        ));
    }

    println!("✓ Worktree removed: {worktree_path}");

    // Delete the branch if auto_clean_branch is enabled
    if auto_clean_branch {
        println!("Deleting branch '{branch_name}'...");
        let output = Command::new("git")
            .args(["branch", "-D", &branch_name])
            .current_dir(&repo_root)
            .output()
            .context("Failed to execute git branch delete command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  Warning: Failed to delete branch '{branch_name}': {stderr}");
            println!("   You may need to delete it manually with: git branch -D {branch_name}");
        } else {
            println!("✓ Branch deleted: {branch_name}");
        }
    } else {
        println!("ℹ️  Branch '{branch_name}' was kept (auto clean disabled)");
    }

    println!();
    println!("✅ Cleanup complete for task '{task_id}'");

    Ok(())
}

pub fn get_matching_worktrees(branch_prefix: &str) -> Result<Vec<(String, String, String)>> {
    let current_dir = std::env::current_dir().context("Could not get current directory")?;
    let repo_root = find_git_repo_root(&current_dir)?;

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo_root)
        .output()
        .context("Failed to execute git worktree list command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git worktree list command failed: {}",
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut current_worktree: Option<(String, String, String)> = None; // (path, head, branch)
    let mut matching_worktrees = Vec::new();

    for line in lines {
        if line.starts_with("worktree ") {
            // If we have a previous worktree, check if it matches and store it
            if let Some((path, head, branch)) = current_worktree.take() {
                if should_clean_worktree(&branch, branch_prefix, &path, &repo_root) {
                    matching_worktrees.push((path, head, branch));
                }
            }

            // Start new worktree
            let path = line.strip_prefix("worktree ").unwrap_or(line);
            current_worktree = Some((path.to_string(), String::new(), String::new()));
        } else if line.starts_with("HEAD ") {
            if let Some((_, ref mut head, _)) = current_worktree.as_mut() {
                let new_head = line.strip_prefix("HEAD ").unwrap_or(line);
                *head = new_head.to_string();
            }
        } else if line.starts_with("branch ") {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                let new_branch = line.strip_prefix("branch ").unwrap_or(line);
                *branch = new_branch.to_string();
            }
        } else if line == "bare" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(bare)".to_string();
            }
        } else if line == "detached" {
            if let Some((_, _, ref mut branch)) = current_worktree.as_mut() {
                *branch = "(detached)".to_string();
            }
        }
    }

    // Handle the last worktree if it exists
    if let Some((path, head, branch)) = current_worktree {
        if should_clean_worktree(&branch, branch_prefix, &path, &repo_root) {
            matching_worktrees.push((path, head, branch));
        }
    }

    Ok(matching_worktrees)
}

pub fn should_clean_worktree(
    branch: &str,
    branch_prefix: &str,
    path: &str,
    repo_root: &std::path::Path,
) -> bool {
    // Clean up branch name by removing refs/heads/ prefix for comparison
    let clean_branch = if branch.starts_with("refs/heads/") {
        branch.strip_prefix("refs/heads/").unwrap_or(branch)
    } else {
        branch
    };

    // Exclude the main repository directory
    let worktree_path = std::path::Path::new(path);
    if worktree_path == repo_root {
        return false;
    }

    // Only include branches that start with the prefix (exclude main/master and special states)
    clean_branch.starts_with(branch_prefix)
}

#[derive(Debug)]
pub struct WorktreeStatus {
    pub has_uncommitted_changes: bool,
    pub has_unpushed_commits: bool,
    pub has_no_remote: bool,
    pub current_branch: String,
    pub remote_branch: Option<String>,
    pub ahead_count: usize,
    pub behind_count: usize,
    pub changed_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub unpushed_commits: Vec<(String, String)>, // (commit_id, message)
    pub is_likely_merged: bool,
    pub merge_info: Option<String>, // e.g., "squash-merged", "merged", "PR #123"
}

impl WorktreeStatus {
    pub fn is_clean(&self) -> bool {
        !self.has_uncommitted_changes
            && (!self.has_unpushed_commits || self.is_likely_merged)
            && (!self.has_no_remote || self.is_likely_merged)
    }

    pub fn get_status_icon(&self) -> &'static str {
        if self.is_clean() {
            "✅"
        } else {
            "⚠️"
        }
    }

    pub fn get_status_details(&self) -> Vec<String> {
        let mut details = Vec::new();

        if self.has_uncommitted_changes {
            details.push("uncommitted changes".to_string());
        }

        if self.has_unpushed_commits && self.ahead_count > 0 {
            details.push(format!("{} unpushed commits", self.ahead_count));
        }

        if self.behind_count > 0 {
            details.push(format!("{} commits behind remote", self.behind_count));
        }

        if self.has_no_remote {
            details.push("no remote tracking branch".to_string());
        }

        details
    }
}

pub fn check_if_branch_merged(branch: &str, worktree_path: &Path) -> (bool, Option<String>) {
    // Try to detect if this branch has been merged into main/master

    // First, find the main branch (main or master)
    let main_branches = ["main", "master"];
    let mut main_branch = None;

    for mb in &main_branches {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", mb])
            .current_dir(worktree_path)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                main_branch = Some(*mb);
                break;
            }
        }
    }

    let main_branch = match main_branch {
        Some(mb) => mb,
        None => return (false, None), // Can't detect without a main branch
    };

    // Method 1: Check if branch is in --merged list (regular merge)
    if let Ok(output) = Command::new("git")
        .args(["branch", "--merged", main_branch])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                let line = line.trim().trim_start_matches('*').trim();
                if line == branch {
                    return (true, Some("merged".to_string()));
                }
            }
        }
    }

    // Method 2: Check if all changes are already in main (squash merge detection)
    // This compares the diff between the merge-base and branch tip
    if let Ok(merge_base_output) = Command::new("git")
        .args(["merge-base", main_branch, branch])
        .current_dir(worktree_path)
        .output()
    {
        if merge_base_output.status.success() {
            let merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
                .trim()
                .to_string();

            // Check if there are any changes between merge-base..branch that aren't in main
            if let Ok(diff_output) = Command::new("git")
                .args(["diff", "--exit-code", &format!("{merge_base}..{branch}")])
                .current_dir(worktree_path)
                .output()
            {
                if diff_output.status.success() {
                    // No diff means no changes
                    return (true, Some("no changes".to_string()));
                }

                // There are changes, check if they're already in main using git log --grep
                // First, get the commit messages from the branch
                if let Ok(log_output) = Command::new("git")
                    .args(["log", "--oneline", &format!("{merge_base}..{branch}")])
                    .current_dir(worktree_path)
                    .output()
                {
                    if log_output.status.success() {
                        let log_str = String::from_utf8_lossy(&log_output.stdout);
                        let commit_count = log_str.lines().count();

                        if commit_count > 0 {
                            // Check if main has any commits that might be squash merges of this branch
                            // Look for commits that mention the branch name or PR
                            if let Ok(main_log) = Command::new("git")
                                .args([
                                    "log",
                                    "--oneline",
                                    "--grep",
                                    &format!("{branch}\\|#[0-9]+"),
                                    &format!("{merge_base}..{main_branch}"),
                                ])
                                .current_dir(worktree_path)
                                .output()
                            {
                                if main_log.status.success() && !main_log.stdout.is_empty() {
                                    return (true, Some("likely squash-merged".to_string()));
                                }
                            }

                            // Alternative: Check if the file changes are already in main
                            // Get list of files changed in the branch
                            if let Ok(files_output) = Command::new("git")
                                .args(["diff", "--name-only", &format!("{merge_base}..{branch}")])
                                .current_dir(worktree_path)
                                .output()
                            {
                                if files_output.status.success() {
                                    let files = String::from_utf8_lossy(&files_output.stdout);
                                    let file_count = files.lines().count();

                                    if file_count > 0 {
                                        // For each file, check if its content in branch matches main
                                        let mut all_changes_in_main = true;

                                        for file in files.lines() {
                                            if !file.is_empty() {
                                                // Compare file content between branch and main
                                                if let Ok(diff) = Command::new("git")
                                                    .args([
                                                        "diff",
                                                        "--no-index",
                                                        "--quiet",
                                                        &format!("{branch}:{file}"),
                                                        &format!("{main_branch}:{file}"),
                                                    ])
                                                    .current_dir(worktree_path)
                                                    .output()
                                                {
                                                    if !diff.status.success() {
                                                        // Files differ
                                                        all_changes_in_main = false;
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        if all_changes_in_main && commit_count > 1 {
                                            // Multiple commits but all changes are in main = likely squash merge
                                            return (
                                                true,
                                                Some("likely squash-merged".to_string()),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Method 3: Try GitHub CLI if available to check PR status
    if let Ok(output) = Command::new("gh")
        .args([
            "pr",
            "list",
            "--state",
            "merged",
            "--head",
            branch,
            "--json",
            "number,title",
        ])
        .current_dir(worktree_path)
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            if json_str.contains("number") {
                // Simple check - if there's a merged PR for this branch
                return (true, Some("PR merged".to_string()));
            }
        }
    }

    (false, None)
}

pub fn check_worktree_status(worktree_path: &Path) -> Result<WorktreeStatus> {
    // Check for uncommitted changes and get file lists
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to execute git status command")?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(anyhow::anyhow!("Git status command failed: {}", stderr));
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    let mut changed_files = Vec::new();
    let mut untracked_files = Vec::new();

    for line in status_str.lines() {
        if line.len() >= 3 {
            let status_code = &line[0..2];
            let file_path = line[3..].trim();

            if status_code.contains('?') {
                untracked_files.push(file_path.to_string());
            } else {
                changed_files.push(format!("{} {}", status_code.trim(), file_path));
            }
        }
    }

    let has_uncommitted_changes = !changed_files.is_empty() || !untracked_files.is_empty();

    // Check for unpushed commits
    let rev_list_output = Command::new("git")
        .args(["rev-list", "--count", "--left-right", "@{u}...HEAD"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to execute git rev-list command")?;

    let (ahead_count, behind_count, has_no_remote) = if rev_list_output.status.success() {
        let output_str = String::from_utf8_lossy(&rev_list_output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();
        if parts.len() == 2 {
            let behind = parts[0].parse::<usize>().unwrap_or(0);
            let ahead = parts[1].parse::<usize>().unwrap_or(0);
            (ahead, behind, false)
        } else {
            (0, 0, false)
        }
    } else {
        // Error might mean no upstream is configured
        (0, 0, true)
    };

    let has_unpushed_commits = ahead_count > 0;

    // Get current branch name
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .context("Failed to get current branch")?;

    let current_branch = if branch_output.status.success() {
        String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string()
    } else {
        "(unknown)".to_string()
    };

    // Get remote branch name
    let remote_branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{u}"])
        .current_dir(worktree_path)
        .output();

    let remote_branch = if let Ok(output) = remote_branch_output {
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Get list of unpushed commits
    let mut unpushed_commits = Vec::new();
    if has_unpushed_commits {
        let log_output = Command::new("git")
            .args(["log", "--oneline", "@{u}..HEAD"])
            .current_dir(worktree_path)
            .output()
            .context("Failed to get unpushed commits")?;

        if log_output.status.success() {
            let log_str = String::from_utf8_lossy(&log_output.stdout);
            for line in log_str.lines() {
                if let Some(pos) = line.find(' ') {
                    let (commit_id, message) = line.split_at(pos);
                    unpushed_commits.push((commit_id.to_string(), message.trim().to_string()));
                }
            }
        }
    }

    // Check if branch is likely merged
    let (is_likely_merged, merge_info) = if has_no_remote {
        check_if_branch_merged(&current_branch, worktree_path)
    } else {
        (false, None)
    };

    Ok(WorktreeStatus {
        has_uncommitted_changes,
        has_unpushed_commits,
        has_no_remote,
        current_branch,
        remote_branch,
        ahead_count,
        behind_count,
        changed_files,
        untracked_files,
        unpushed_commits,
        is_likely_merged,
        merge_info,
    })
}
