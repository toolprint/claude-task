# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Claude Task is a Rust CLI tool that creates isolated development environments for AI-assisted coding. It integrates macOS keychain extraction, git worktree management, and Docker containerization to run Claude Code in secure, isolated environments.

## Core Architecture

The project consists of five main modules:

- **`main.rs`**: CLI interface using clap, orchestrates all operations
- **`credentials.rs`**: macOS keychain extraction and biometric authentication
- **`docker.rs`**: Docker container and volume management using Bollard API
- **`mcp.rs`**: MCP (Model Context Protocol) server implementation exposing CLI functionality as tools
- **`permission.rs`**: Approval tool permission validation for secure MCP operations

### Key Data Flow

1. **Setup Phase**: Extract macOS credentials → Create Docker volumes → Configure environment
2. **Task Phase**: Create git worktree → Build/validate Docker image → Run containerized Claude
3. **Cleanup Phase**: Remove worktrees and volumes
4. **MCP Mode**: Start stdio server → Expose CLI operations as MCP tools → Validate permissions

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

# Run MCP server for Claude Code integration
just run mcp
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
- Functions in `main.rs` handle worktree lifecycle

### Docker Integration
- Uses Bollard library for Docker API communication
- Creates shared volumes: `claude-task-home`, `claude-task-npm-cache`, `claude-task-node-cache`
- Container configuration in `docker.rs`
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

### MCP Server Architecture
- Implements full MCP (Model Context Protocol) server using `rmcp` crate
- Exposes all CLI functionality as MCP tools for Claude Code integration
- Uses stdio transport for direct communication with Claude Code
- Validates approval tool permissions with format `mcp__<server_name>__<tool_name>`
- Provides structured error handling and parameter validation

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
- MCP approval tool permission validation prevents unauthorized operations

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
- `rmcp`: MCP (Model Context Protocol) server implementation
- `serde`: JSON serialization for MCP tools
- `tracing`: Logging for MCP server

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

### Running Specific Tests
```bash
# Run MCP-specific tests
cargo test mcp

# Run with debug output
cargo test -- --nocapture

# Run a single test
cargo test test_mcp_config_validation
```

## Build System Details

### Build Script (`build.rs`)
- Parses `src/mcp.rs` to extract MCP tool definitions at compile time
- Generates `mcp_help.rs` with tool documentation
- Uses `syn` crate for AST parsing and `quote` for code generation

### Multi-platform Docker Build
- Uses `docker-bake.hcl` for buildx configuration
- Supports both AMD64 and ARM64 architectures
- Multi-stage Dockerfile optimizes image size

## MCP Server Development

### Running the MCP Server
```bash
# Start MCP server (stdio mode)
cargo run -- mcp

# Start with debug logging
RUST_LOG=debug cargo run -- mcp

# Test MCP server with a real task
just task "test prompt" --approval-tool-permission "mcp__approval_server__approve_command"
```

### MCP Tool Development
- All CLI commands are exposed as MCP tools in `src/mcp.rs`
- Tool parameters use JSON Schema for validation
- Approval tool permissions must follow format: `mcp__<server_name>__<tool_name>`
- Global options (debug, worktree-base-dir, etc.) are embedded in each tool
- MCP config files can be passed through and mounted in containers
- Build script auto-generates help text from tool definitions

## Common Development Tasks

### Adding a New CLI Command
1. Add the command variant to the `Commands` enum in `main.rs`
2. Implement the command handler in the appropriate module
3. If exposing via MCP, add corresponding tool in `mcp.rs`
4. Update tests if needed

### Debugging Docker Container Issues
```bash
# Run with debug logging
RUST_LOG=debug just run <command>

# Inspect Docker volumes
docker volume ls | grep claude-task
docker volume inspect claude-task-home

# Run container interactively for debugging
docker run -it --rm \
  -v claude-task-home:/home/base:ro \
  -v $(pwd):/workspace \
  ghcr.io/anthropics/claude-code-docker-base \
  /bin/bash
```