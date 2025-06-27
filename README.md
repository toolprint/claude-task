# Claude Setup

A CLI workspace setup utility for extracting macOS settings, credentials, and managing git worktrees for Claude development environments.

## Purpose

This tool helps bridge the gap between your local macOS development environment and containerized or remote development environments by safely extracting and packaging necessary configuration files, credentials, and settings. It also provides git worktree management for isolated development sessions.

## Features

### Credential Setup

- Extracts macOS keychain credentials for Claude Code
- Filters and packages Claude configuration files
- Prepares mount-ready configuration bundles for containers

### Worktree Management

- Creates isolated git worktrees for development sessions
- Generates timestamped worktree directories
- Automatically creates feature branches with sanitized names
- Stores worktrees in `.claude-task/worktrees/` by default (configurable)

### Docker Volume Management

- Creates named Docker volumes for containerized development environments
- Copies Claude configuration and credentials to volumes
- Supports refreshing credentials before volume initialization
- Generates volume names in format: `claude-task-<task-id>-home-dir`
- Uses native Docker API for improved performance and reliability
- Eliminates dependency on docker-compose YAML files

## Usage

### Extract and Setup Credentials

```bash
# Extract all supported settings and credentials
cargo run -- setup
```

This will create:

- `./output/.claude/.credentials.json` - Extracted keychain credentials
- `./output/.claude.json` - Filtered Claude configuration

### Manage Git Worktrees

```bash
# Create a new worktree with default branch prefix "claude-task/"
cargo run -- worktree my-session-name

# Create worktree with custom branch prefix
cargo run -- worktree my-session-name --branch-prefix "feature-"

# Create worktree in custom directory
cargo run -- worktree my-session-name --worktree-dir "/path/to/worktrees"

# List current worktrees (filtered by default prefix "claude-task/")
cargo run -- list

# List worktrees with custom prefix filter
cargo run -- list --branch-prefix "feature-"

# List all worktrees (no filter)
cargo run -- list --branch-prefix ""

# Remove and clean up a worktree by session ID
cargo run -- remove my-session-name

# Remove worktree with custom branch prefix
cargo run -- remove my-session-name --branch-prefix "feature-"

# Initialize a docker volume for a task
cargo run -- init-volume my-task-id

# Initialize a docker volume and refresh credentials first
cargo run -- init-volume my-task-id --refresh-credentials
```

This will:

- Find the git repository root
- Create a new branch (e.g., `claude-task/my-session-name`)
- Create worktree in `.claude-task/worktrees/my-session-name_<timestamp>/`

### Initialize Docker Volume

```bash
# Initialize a docker volume for a task
cargo run -- init-volume my-task-id

# Initialize a docker volume and refresh credentials first
cargo run -- init-volume my-task-id --refresh-credentials
```

This will:

- Create a Docker volume named `claude-task-my-task-id-home-dir`
- Copy all files from `./output/` to the volume under `/home/node`
- Set appropriate permissions (writable by all)
- Optionally refresh credentials before copying if `--refresh-credentials` is used

### Run Claude Task

```bash
# Run a Claude task in docker container (auto-generates task ID, creates worktree by default)
cargo run -- run-task "Analyze the codebase and suggest improvements"

# Run a Claude task with specific task ID
cargo run -- run-task "Review the API design" --task-id my-custom-task

# Run a Claude task and build the image first
cargo run -- run-task "Review the code" --build

# Use current directory instead of creating a worktree
cargo run -- run-task "Quick analysis" --use-current-dir

# Use a custom workspace directory
cargo run -- run-task "Analyze specific project" --workspace-dir "/path/to/project"
```

This will:

- Generate a short task ID (or use provided one)
- **By default**: Create a git worktree for the task (branch: `claude-task/{task-id}`)
- Initialize docker volume with credentials
- Run Docker container with Claude
- Mount the worktree (or specified directory) as the workspace
- Pass the prompt to Claude
- Provide isolated environment with persistent home directory

**Workspace Options:**
- **Default**: Creates git worktree in `.claude-task/worktrees/{task-id}_{timestamp}/`
- **`--use-current-dir`**: Uses the current directory as workspace
- **`--workspace-dir`**: Uses the specified custom directory as workspace

### Command Reference

```bash
# Show help (default behavior)
cargo run
cargo run -- --help

# Setup credentials
cargo run -- setup

# Create git worktree
cargo run -- worktree <session-name> [--branch-prefix <prefix>] [--worktree-dir <directory>]

# List current worktrees (filtered by default prefix "claude-task/")
cargo run -- list [--branch-prefix <prefix>]

# Remove and clean up a worktree
cargo run -- remove <session-id> [--branch-prefix <prefix>]

# Initialize a docker volume for a task
cargo run -- init-volume <task-id> [--refresh-credentials]

# Run a Claude task in docker container
cargo run -- run-task <prompt> [--task-id <task-id>] [--build] [--use-current-dir] [--workspace-dir <directory>]

# List Docker volumes
cargo run -- list-volumes

# Clean up Docker volumes for a task
cargo run -- clean-volumes <task-id>
```

## Requirements

- macOS 10.15 or later
- Rust 1.70+
- Administrative privileges for keychain access
- Git repository for worktree operations
