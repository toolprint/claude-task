# Claude Task

A CLI tool for setting up isolated development environments with Docker and git worktrees for Claude Code tasks.

## Description

Claude Task is a Rust-based CLI utility that streamlines the creation of isolated development environments for AI-assisted coding tasks. It extracts macOS credentials, manages git worktrees, handles Docker volumes, and runs Claude Code in containerized environments with proper authentication and workspace isolation.

## Features

### Environment Setup
- Extracts macOS keychain credentials for Claude Code authentication
- Configures Claude settings for containerized environments
- Sets up isolated home directories with proper permissions

### Git Worktree Management
- Creates isolated git worktrees for development sessions
- Generates timestamped worktree directories
- Automatically creates feature branches with sanitized names
- Configurable branch prefixes and worktree locations

### Docker Integration
- Creates and manages Docker volumes for persistent environments
- Builds and runs containerized Claude Code instances
- Mounts workspaces with proper permissions
- Handles credential injection and environment setup

### Task Execution
- Runs Claude Code tasks in isolated Docker containers
- Supports custom prompts and task configurations
- Provides workspace flexibility (worktrees, current directory, or custom paths)
- Automatic cleanup and resource management

## Installation

### Prerequisites
- macOS 10.15 or later
- Rust 1.70+
- Docker Desktop
- Git
- Administrative privileges for keychain access

### Build from Source
```bash
# Clone the repository
git clone <repository-url>
cd claude-task

# Build the project
just build

# Install locally (creates symlink 'ct' for convenience)
just install
```

## Usage

### Command Overview
```bash
# Show all available commands
claude-task --help

# Setup credentials and environment
claude-task setup

# Git worktree management
claude-task worktree <command>

# Docker volume management  
claude-task volume <command>

# Run Claude tasks
claude-task run <prompt>

# Clean up resources
claude-task clean
```

### Basic Workflow

1. **Initial Setup**
   ```bash
   # Extract credentials and setup environment
   claude-task setup
   ```

2. **Run a Task**
   ```bash
   # Run Claude with a prompt (creates worktree automatically)
   claude-task run "Analyze this codebase and suggest improvements"
   
   # Run with custom task ID
   claude-task run "Review the API design" --task-id my-review
   
   # Use current directory instead of creating worktree
   claude-task run "Quick code review" --use-current-dir
   ```

3. **Manual Worktree Management**
   ```bash
   # Create a worktree manually
   claude-task worktree create my-feature
   
   # List existing worktrees
   claude-task worktree list
   
   # Remove a worktree
   claude-task worktree remove my-feature
   ```

4. **Cleanup**
   ```bash
   # Clean up all resources (worktrees and volumes)
   claude-task clean
   ```

### Global Options
- `--worktree-base-dir`: Base directory for worktrees (default: `~/.claude-task/worktrees`)
- `--branch-prefix`: Branch prefix for worktrees (default: `claude-task/`)
- `--task-base-home-dir`: Base directory for task environments (default: `~/.claude-task/home`)
- `--debug`: Enable debug mode

## Development

### Using Just
This project uses [Just](https://just.systems/) as a command runner:

```bash
# Show available commands
just

# Build the project
just build

# Run with arguments
just run --help

# Run tests
just test

# Format code
just fmt

# Run linter
just clippy

# Install locally
just install

# Run a task (shortcut)
just task "Your prompt here"
```

### Project Structure
- `src/main.rs` - Main CLI entry point
- `src/credentials.rs` - macOS keychain credential extraction
- `src/docker.rs` - Docker volume and container management
- `tests/` - Integration tests
- `Dockerfile` - Container image definition
- `justfile` - Development commands

## Configuration

### Default Locations
- Worktrees: `~/.claude-task/worktrees/`
- Task home directories: `~/.claude-task/home/`
- Branch prefix: `claude-task/`

### Environment Variables
The tool respects standard environment variables:
- `CARGO_HOME` - For binary installation location
- `DOCKER_HOST` - For Docker daemon connection

## Troubleshooting

### Common Issues
- **Keychain access denied**: Ensure administrative privileges and grant access when prompted
- **Docker connection failed**: Verify Docker Desktop is running
- **Git worktree creation failed**: Ensure you're in a git repository
- **Permission errors**: Check file system permissions in worktree directories

### Debug Mode
Use `--debug` flag for verbose output:
```bash
claude-task --debug run "Debug this issue"
```
