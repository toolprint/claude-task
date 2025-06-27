# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Claude Task is a Rust CLI tool that creates isolated development environments for AI-assisted coding. It integrates macOS keychain extraction, git worktree management, and Docker containerization to run Claude Code in secure, isolated environments.

## Core Architecture

The project consists of three main modules:

- **`main.rs`**: CLI interface using clap, orchestrates all operations
- **`credentials.rs`**: macOS keychain extraction and biometric authentication
- **`docker.rs`**: Docker container and volume management using Bollard API

### Key Data Flow

1. **Setup Phase**: Extract macOS credentials → Create Docker volumes → Configure environment
2. **Task Phase**: Create git worktree → Build/validate Docker image → Run containerized Claude
3. **Cleanup Phase**: Remove worktrees and volumes

## Development Commands

### Building and Testing
```bash
# Build project
just build

# Build release version
just build-release

# Run tests
just test

# Check code without building
just check

# Format code
just fmt

# Run clippy linter
just clippy
```

### Running the CLI
```bash
# Run with arguments
just run --help
just run setup
just run worktree create my-task

# Install locally (creates 'ct' symlink)
just install

# Run a task directly
just task "Your prompt here"
```

### Docker Operations
```bash
# Build Docker image
just build-docker-image

# Manual Docker build
docker buildx bake
```

## Architecture Details

### Git Worktree Management
- Creates timestamped branches with `claude-task/` prefix
- Stores worktrees in `~/.claude-task/worktrees/` by default
- Handles branch cleanup and worktree removal
- Functions in `main.rs:172-473` handle worktree lifecycle

### Docker Integration
- Uses Bollard library for Docker API communication
- Creates shared volumes: `claude-task-home`, `claude-task-npm-cache`, `claude-task-node-cache`
- Container configuration in `docker.rs:254-389`
- Mounts workspace, credentials, and optional MCP configs

### Credential Management
- Extracts macOS keychain using `keyring` crate
- Implements biometric authentication with Touch ID/Face ID
- Filters and packages Claude configuration files
- Creates read-only bind mounts for container access

### Container Environment
- Based on Node.js 22 with development tools
- Installs Claude Code globally via npm
- Sets up zsh with powerline10k theme
- Mounts workspaces and credentials securely

## Configuration

### Default Locations
- Worktrees: `~/.claude-task/worktrees/`
- Task home: `~/.claude-task/home/`
- Branch prefix: `claude-task/`

### Docker Volumes
- `claude-task-home`: Contains credentials and config (read-only)
- `claude-task-npm-cache`: Shared npm cache
- `claude-task-node-cache`: Shared node cache

### Container Mounts
- `/workspace`: Project workspace (bind mount)
- `/home/base`: Credentials volume (read-only)
- `/home/node/.npm`: npm cache volume
- `/home/node/.cache`: node cache volume

## Key Implementation Notes

### Error Handling
- Uses `anyhow` for error context throughout
- Graceful fallbacks for missing Docker images
- User confirmation prompts for dangerous operations

### Security Considerations
- Biometric authentication for keychain access
- Read-only credential mounts
- Permission prompts for dangerous operations
- Isolated container environments

### Performance Optimizations
- Shared npm/node cache volumes
- Reusable Docker images
- Parallel volume creation
- Stream-based Docker operations

## Dependencies

### Core Dependencies
- `clap`: CLI argument parsing
- `anyhow`: Error handling
- `tokio`: Async runtime
- `bollard`: Docker API client
- `keyring`: macOS keychain access
- `localauthentication-rs`: Biometric authentication

### Container Dependencies
- Node.js 22 base image
- Claude Code npm package
- Development tools (git, ripgrep, fzf, etc.)
- zsh with powerline10k theme

## Testing

Run tests with:
```bash
just test
```

Test files are in `tests/` directory and include MCP integration tests.