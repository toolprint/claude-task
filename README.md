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

### HT-MCP Integration
- Web-based terminal interface for monitoring Claude's command executions
- Real-time terminal session viewing via web browser
- NGINX proxy with WebSocket support for reliable connections
- Secure terminal access with session management
- Transparent command execution tracking

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

# Build with HT-MCP support
just sync-modules              # Initialize HT-MCP submodule
just build-ht-mcp              # Build HT-MCP binaries
just docker-bake               # Build Docker image with HT-MCP

# Install locally (creates symlink 'ct' for convenience)
just install
```

## Usage

### Command Overview
```bash
# Show all available commands
claude-task --help

# Setup credentials and environment
claude-task setup  # or: claude-task s

# Git worktree management
claude-task worktree <command>  # or: claude-task wt <command>

# Docker volume management  
claude-task docker <command>  # or: claude-task d <command>

# Run Claude tasks
claude-task run <prompt>  # or: claude-task r <prompt>

# Run with HT-MCP web terminal (recommended)
just run-ht-mcp                    # Use default comprehensive prompt
just run-ht-mcp-debug              # Same with debug output
just run-ht-mcp port=8080          # Custom port
just run-ht-mcp prompt="Custom task" port=3618  # Custom prompt and port

# Clean up resources
claude-task clean  # or: claude-task c

# Start MCP server (for Claude Code integration)
claude-task mcp

# Show version information
claude-task version  # or: claude-task v
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
- `init_docker_volume` - Initialize Docker volumes
- `list_docker_volume` - List Docker volumes
- `clean_docker_volume` - Clean Docker volumes
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

2. **Docker Volume Management** (Optional - automatically handled by `run`)
   ```bash
   # Initialize Docker volumes
   claude-task docker init  # or: claude-task d i
   
   # List Docker volumes
   claude-task docker list  # or: claude-task d l
   
   # Clean Docker volumes
   claude-task docker clean  # or: claude-task d c
   ```

3. **Run a Task**
   ```bash
   # Run Claude with a prompt (creates worktree automatically)
   claude-task run "Analyze this codebase and suggest improvements"
   
   # Run with custom task ID
   claude-task run "Review the API design" --task-id my-review
   
   # Use current directory instead of creating worktree
   claude-task run "Quick code review" --workspace-dir
   
   # Run with MCP configuration and approval tool permission
   claude-task run "Implement new feature" \
     --mcp-config ./mcp-servers.json \
     --approval-tool-permission "mcp__approval_server__approve_command"
   
   # Run with HT-MCP web terminal interface (recommended)
   just run-ht-mcp  # Uses default comprehensive development workflow
   
   # Access the web interface during execution:
   # Direct HT-MCP: http://localhost:3618
   # Via NGINX proxy: http://localhost:4618 (recommended)
   ```

4. **Manual Worktree Management**
   ```bash
   # Create a worktree manually
   claude-task worktree create my-feature  # or: claude-task wt c my-feature
   
   # List existing worktrees
   claude-task worktree list  # or: claude-task wt l
   
   # Remove a worktree
   claude-task worktree remove my-feature  # or: claude-task wt rm my-feature
   
   # Open a worktree in your IDE
   claude-task worktree open  # or: claude-task wt o
   ```

5. **Cleanup**
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
```
claude-task/
├── src/                            # Rust source code
│   ├── main.rs                     # Main CLI entry point
│   ├── credentials.rs              # macOS keychain credential extraction
│   ├── docker.rs                   # Docker volume and container management
│   ├── mcp.rs                      # MCP (Model Context Protocol) server implementation
│   └── permission.rs               # Approval tool permission validation
├── docker/                         # Docker configuration
│   ├── Dockerfile                  # Multi-stage container build
│   ├── docker-bake.hcl             # Docker buildx configuration
│   ├── entrypoint.sh               # Container initialization with HT-MCP
│   └── nginx/
│       └── ht-mcp-proxy.conf       # NGINX WebSocket proxy for HT-MCP
├── scripts/                        # Execution scripts
│   ├── run-with-ht-mcp.sh          # Main HT-MCP runner script
│   ├── test-ht-mcp.sh              # Setup and build script
│   ├── test-docker.sh              # Docker testing
│   └── default-ht-mcp-prompt.txt   # Default comprehensive test prompt
├── examples/                       # Examples and testing
│   └── local-nginx-test/           # Local NGINX testing setup
├── tests/                          # Integration tests
├── modules/ht-mcp/                 # HT-MCP submodule
└── justfile                        # Development commands

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

### HT-MCP Specific Issues
- **HT-MCP binary not found**: Ensure the HT-MCP submodule is properly initialized
- **NGINX proxy fails to start**: Verify port 4618 is available, check for port conflicts
- **Web interface not accessible**: Ensure `enableWebServer: true` was used when creating sessions
- **Port conflicts**: Check if ports 3618/4618 are already in use
- **WebSocket connection errors**: Use the NGINX proxy on port 4618 for better reliability

### Debug Mode
Use `--debug` flag for verbose output:
```bash
claude-task --debug run "Debug this issue"

# Debug HT-MCP integration
just run-ht-mcp-debug  # Includes detailed logging for HT-MCP operations
```

## HT-MCP Integration Details

### Architecture

HT-MCP (Headless Terminal MCP Server) provides a web-based terminal interface for transparent command execution monitoring:

1. **HT-MCP Server**: Provides MCP tools for terminal session management and web interface
2. **NGINX Proxy**: Handles WebSocket connections and CORS for the web interface
3. **CCO Approval Tool**: Ensures Claude uses HT-MCP instead of built-in tools
4. **Claude Task Container**: Sandboxed environment with all components integrated

### HT-MCP Tools

When HT-MCP is enabled, Claude has access to these terminal management tools:

#### Session Management
- `ht_create_session`: Create new terminal session (always use `enableWebServer: true`)
- `ht_close_session`: Close terminal session
- `ht_list_sessions`: List active sessions

#### Terminal Interaction
- `ht_execute_command`: Execute command and return output
- `ht_send_keys`: Send keystrokes to terminal
- `ht_take_snapshot`: Capture terminal state

### Local Testing

Test the NGINX proxy configuration locally:

```bash
# Start local test environment
just test-nginx-local

# Or manually:
cd examples/local-nginx-test
./start-nginx.sh  # Terminal 1
# Start Claude Code with HT-MCP in Terminal 2
```

### Security Features

- **Permission Validation**: CCO approval tool validates all tool requests
- **Restricted Built-in Tools**: When HT-MCP is enabled, Claude's built-in tools are restricted
- **Session Isolation**: Each task runs in its own containerized environment
- **Transparent Monitoring**: All terminal operations are visible via web interface

## Contributing

When contributing to this project:

1. Follow the existing project structure
2. Update documentation for any new features
3. Test both local and Docker environments
4. Ensure backward compatibility with existing scripts

For HT-MCP integration contributions:
- Test the web interface thoroughly
- Verify NGINX proxy configuration
- Ensure WebSocket connections work reliably
- Update integration tests as needed

## References

- [HT-MCP Repository](https://github.com/cripplet/ht-mcp) - Headless Terminal MCP Server
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP Specification
- [Claude Code Documentation](https://docs.anthropic.com/claude-code) - Official Claude Code docs
- [Just](https://just.systems/) - Command runner used by this project
