# Claude Task

A CLI tool for setting up isolated development environments with Docker and git worktrees for Claude Code tasks. Also provides an MCP (Model Context Protocol) server for seamless integration with Claude Code.

## Description

Claude Task is a Rust-based CLI utility that streamlines the creation of isolated development environments for AI-assisted coding tasks. It extracts macOS credentials, manages git worktrees, handles Docker volumes, and runs Claude Code in containerized environments with proper authentication and workspace isolation. Additionally, it provides an MCP server that exposes all functionality as tools for direct integration with Claude Code.

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
- MCP configuration support for enhanced Claude Code integration

### MCP Server Integration
- Provides a full MCP (Model Context Protocol) server implementation
- Exposes all CLI functionality as MCP tools for Claude Code
- Supports stdio transport for direct integration
- Enables seamless task management from within Claude Code sessions
- Validates approval tool permissions for secure operations

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

# Start MCP server (for Claude Code integration)
claude-task mcp
```

### MCP Server Usage

The MCP (Model Context Protocol) server allows direct integration with Claude Code, exposing all claude-task functionality as tools.

#### Starting the MCP Server
```bash
# Start the MCP server (listens on stdio)
claude-task mcp
```

#### Available MCP Tools
The MCP server exposes the following tools for use within Claude Code:

- `setup` - Setup credentials and environment
- `create_worktree` - Create a git worktree for a task
- `list_worktree` - List current git worktrees  
- `remove_worktree` - Remove and clean up a worktree
- `init_volume` - Initialize Docker volumes
- `list_volume` - List Docker volumes
- `clean_volume` - Clean Docker volumes
- `run_task` - Run a Claude task in a Docker container
- `clean` - Clean up all worktrees and volumes

#### MCP Configuration
You can pass MCP configuration files to tasks using the `--mcp-config` flag:

```bash
# Run with MCP config file
claude-task run "Analyze this code" --mcp-config ./mcp-config.json
```

Example MCP configuration file:
```json
{
  "mcpServers": {
    "context7": {
      "type": "sse", 
      "url": "https://mcp.context7.com/sse"
    }
  }
}
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
   
   # Run with MCP configuration and approval tool permission
   claude-task run "Implement new feature" \
     --mcp-config ./mcp-servers.json \
     --approval-tool-permission "mcp__approval_server__approve_command"
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
- `src/mcp.rs` - MCP (Model Context Protocol) server implementation
- `src/permission.rs` - Approval tool permission validation
- `tests/` - Integration tests including MCP functionality
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
- `RUST_LOG` - For MCP server logging (e.g., `RUST_LOG=debug`)

### MCP Server Configuration
When running the MCP server (`claude-task mcp`), the following apply:
- Server listens on stdio for MCP protocol communication
- All CLI functionality is exposed as MCP tools
- Approval tool permissions are validated for security
- Debug logging can be enabled with `RUST_LOG=debug claude-task mcp`

## Troubleshooting

### Common Issues
- **Keychain access denied**: Ensure administrative privileges and grant access when prompted
- **Docker connection failed**: Verify Docker Desktop is running
- **Git worktree creation failed**: Ensure you're in a git repository
- **Permission errors**: Check file system permissions in worktree directories
- **MCP server connection issues**: Ensure the server is running with `claude-task mcp`
- **Invalid approval tool permission**: Use format `mcp__<server_name>__<tool_name>`
- **MCP config file not found**: Verify the path and file exists

### Debug Mode
Use `--debug` flag for verbose output:
```bash
claude-task --debug run "Debug this issue"
```
