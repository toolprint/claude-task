# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Claude Task is a Rust CLI tool that creates isolated development environments for AI-assisted coding. It integrates macOS keychain extraction, git worktree management, and Docker containerization to run Claude Code in secure, isolated environments.

## Core Architecture

The project consists of six main modules:

- **`main.rs`**: CLI interface using clap, orchestrates all operations
- **`credentials.rs`**: macOS keychain extraction and biometric authentication
- **`credential_sync.rs`**: Synchronization manager to minimize biometric prompts for parallel tasks
- **`docker.rs`**: Docker container and volume management using Bollard API
- **`mcp.rs`**: MCP (Model Context Protocol) server implementation exposing CLI functionality as tools
- **`permission.rs`**: Approval tool permission validation for secure MCP operations
- **`config.rs`**: Configuration management for persistent settings
- **`handle_config.rs`**: Config file operations (init, edit, validate, show)

### Key Data Flow

1. **Setup Phase**: Extract macOS credentials → Sync with lock mechanism → Create Docker volumes → Configure environment
2. **Task Phase**: Create git worktree → Build/validate Docker image → Run containerized Claude
3. **Cleanup Phase**: Remove worktrees (with status checking) and volumes
4. **MCP Mode**: Start stdio server → Expose CLI operations as MCP tools → Validate permissions

### Credential Synchronization

When multiple tasks run in parallel:
- First task acquires file-based lock and extracts credentials
- Other tasks wait up to 60 seconds for sync completion
- 5-minute validation window prevents redundant biometric prompts
- Automatic retry on credential errors with fresh extraction
- Metadata stored in `{task_base_home_dir}/.credential_metadata/`

## Development Commands

### Building and Testing
```bash
# Build project
just build

# Build release version  
just build-release

# Run tests
just test

# Run specific test
cargo test test_name

# Check code without building
just check

# Format code
just fmt

# Run clippy linter
just clippy

# Pre-commit validation (runs all checks)
just pre-commit
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
# Build Docker image (without HT-MCP)
just docker-bake

# Build with HT-MCP support
just prepare-docker-with-ht-mcp
just docker-bake-with-ht-mcp

# Test Docker setup
just test-docker
```

### HT-MCP Development
```bash
# Sync and build HT-MCP submodule
just sync-modules
just build-ht-mcp

# Run with HT-MCP web interface
just run-ht-mcp
just run-ht-mcp-debug

# Test NGINX proxy locally
just test-nginx-local
```

## Architecture Details

### Git Worktree Management
- Creates timestamped branches with `claude-task/` prefix
- Stores worktrees in `~/.claude-task/worktrees/` by default
- Status checking detects uncommitted changes, unpushed commits, missing remotes
- Smart cleanup separates clean vs unclean worktrees (--force required for unclean)
- Squash-merge detection prevents false positive "unclean" status

### Docker Integration
- Uses Bollard library for async Docker API communication
- Creates shared volumes: `claude-task-home`, `claude-task-npm-cache`, `claude-task-node-cache`
- Supports both sync and async task execution modes
- Optional HT-MCP integration for web-based terminal monitoring
- NGINX proxy on port 4618 for reliable WebSocket connections

### Configuration System
- Persistent config at `~/.claude-task/config.json`
- Supports custom paths, Docker settings, and defaults
- Config commands: init, edit, show, validate
- Command-line args override config values

### MCP Server Architecture
- Uses `rmcp` crate for MCP protocol implementation
- All CLI commands exposed as MCP tools with JSON Schema validation
- Approval tool permissions format: `mcp__<server_name>__<tool_name>`
- Build script (`build.rs`) auto-generates tool documentation
- Global options embedded in each tool's parameters

## Key Implementation Notes

### Error Handling
- Uses `anyhow` for error context throughout
- Credential errors trigger automatic retry with re-sync
- Graceful fallbacks for missing Docker images
- User confirmation prompts for dangerous operations

### Security Considerations
- Biometric authentication for keychain access
- Read-only credential mounts in containers
- No credentials stored in sync metadata (only hashes)
- MCP approval tool validation prevents unauthorized operations
- Permission prompts for destructive actions

### Performance Optimizations
- Shared npm/node cache volumes across tasks
- Credential sync reduces redundant keychain access
- Parallel Docker volume creation
- Stream-based log parsing for real-time output

## Testing

### Running Tests
```bash
# All tests
just test

# Specific test module
cargo test credential_sync

# With output
cargo test -- --nocapture

# Single test
cargo test test_parallel_sync_with_lock
```

### Test Organization
- Unit tests in each module (bottom of source files)
- Integration tests in `tests/` directory
- MCP integration tests in `tests/mcp.rs`
- Credential sync tests cover parallel execution scenarios

## Common Development Tasks

### Adding a New CLI Command
1. Add command variant to `Commands` enum in `main.rs`
2. Implement handler in appropriate module or create new module
3. If exposing via MCP, add `#[tool(description = "...")]` decorated method in `mcp.rs`
4. Update tests and run `just pre-commit`

### Debugging Docker Container Issues
```bash
# Run with debug logging
RUST_LOG=debug just run <command>

# Inspect Docker volumes
docker volume ls | grep claude-task
docker volume inspect claude-task-home

# Run container interactively
docker run -it --rm \
  -v claude-task-home:/home/base:ro \
  -v $(pwd):/workspace \
  ghcr.io/anthropics/claude-code-docker-base \
  /bin/bash
```

### Working with Credential Sync
```bash
# Check sync metadata
ls -la ~/.claude-task/home/.credential_metadata/

# Debug sync issues
RUST_LOG=debug just run setup --debug

# Force credential refresh
rm -rf ~/.claude-task/home/.credential_metadata/
```

## Build System Details

### Multi-stage Dockerfile
- Base stage: Node.js 22 with development tools
- Optional HT-MCP stage adds terminal server binaries
- Final stage configures user environment and entrypoint

### Build Script (`build.rs`)
- Parses `src/mcp.rs` AST to extract tool definitions
- Generates `mcp_help.rs` with tool documentation
- Uses `syn` for parsing and `quote` for code generation
- Runs at compile time, output in `target/debug/build/*/out/`

### Platform Support
- Primary: macOS (with keychain and biometric auth)
- Docker images: linux/amd64 and linux/arm64
- HT-MCP binaries required for both architectures