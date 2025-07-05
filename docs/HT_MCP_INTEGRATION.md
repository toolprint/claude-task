# HT-MCP Integration Guide

This document describes the integration of HT-MCP (headless terminal MCP server) with claude-task, enabling web-based terminal monitoring and interaction.

## Overview

HT-MCP provides a web-based terminal interface that allows users to view and interact with Claude's command executions in real-time. This integration adds transparency and security to AI-assisted development workflows.

## Architecture

### Components

1. **HT-MCP Server**: Provides MCP tools for terminal session management and a web interface
2. **NGINX Proxy**: Handles WebSocket connections and CORS for the web interface
3. **CCO Approval Tool**: Ensures Claude uses HT-MCP instead of built-in tools
4. **Claude Task Container**: Sandboxed environment with all components integrated

### Data Flow

1. Claude Code connects to HT-MCP via MCP protocol
2. HT-MCP creates terminal sessions and exposes web interface on port 3618
3. NGINX proxies the web interface to port 4618 with proper WebSocket support
4. Users can monitor Claude's terminal activity via the web interface

## Quick Start

### Prerequisites

- Rust toolchain installed
- Docker installed and running
- Node.js (for local testing)

### Building

```bash
# Build the claude-task binary
just build-release

# Build the Docker image
docker buildx bake -f docker/docker-bake.hcl
```

### Running

```bash
# Using justfile targets (recommended)
just run-ht-mcp                    # Use default comprehensive prompt
just run-ht-mcp-debug              # Same with debug output
just run-ht-mcp port=8080          # Custom port
just run-ht-mcp prompt="Custom task" port=3618  # Custom prompt and port

# Direct script usage
cd scripts
./run-with-ht-mcp.sh -a            # Use default prompt from file
./run-with-ht-mcp.sh -a -d         # With debug output
./run-with-ht-mcp.sh -a 8080 "Create a Python hello world application"  # Custom
```

### Web Interface

Once running, the HT-MCP web interface will be available at:
- **Direct access**: `http://localhost:3618`
- **Via NGINX proxy**: `http://localhost:4618` (recommended)

## Configuration

### MCP Server Setup

The integration automatically configures two MCP servers:

1. **CCO Approval Server**: Validates tool permissions
   ```json
   {
     "type": "http",
     "url": "https://auth-server-cco-mcp-873660917363.us-west1.run.app/mcp"
   }
   ```

2. **HT-MCP Server**: Provides terminal tools
   ```json
   {
     "command": "ht-mcp",
     "args": ["--debug"]
   }
   ```

### NGINX Configuration

The NGINX proxy configuration (`docker/nginx/ht-mcp-proxy.conf`) provides:
- WebSocket support for terminal connections
- CORS headers for cross-origin access
- Proper timeout settings for long-running sessions

## Available Tools

HT-MCP provides these MCP tools for Claude:

### Session Management
- `ht_create_session`: Create new terminal session
- `ht_close_session`: Close terminal session
- `ht_list_sessions`: List active sessions

### Terminal Interaction
- `ht_execute_command`: Execute command and return output
- `ht_send_keys`: Send keystrokes to terminal
- `ht_take_snapshot`: Capture terminal state

### Key Parameters

When creating sessions, always use:
```json
{
  "enableWebServer": true
}
```

This enables the web interface on port 3618.

## Security Features

### Permission Validation

The CCO approval tool validates all tool requests with the format:
```
mcp__<server_name>__<tool_name>
```

Example: `mcp__cco__approval_prompt`

### Restricted Built-in Tools

When HT-MCP is enabled, Claude's built-in tools (Bash, Edit, Write) are restricted to encourage use of the monitored HT-MCP interface.

## Local Testing

For development and testing without Docker:

```bash
cd examples/local-nginx-test

# Terminal 1: Start NGINX proxy
./start-nginx.sh

# Terminal 2: Start Claude Code or MCP Client
# Start Claude Code or another long-running MCP client to use the HT-MCP binary 
# to launch a session with a web server. We don't have a script for this in this 
# repo at this time.
```

This setup allows testing the NGINX proxy configuration and HT-MCP integration locally using your preferred MCP client.

## Troubleshooting

### Common Issues

1. **HT-MCP binary not found**
   - Ensure the HT-MCP submodule is properly initialized
   - Check that the binary exists in `modules/ht-mcp/release/latest/`

2. **NGINX proxy fails to start**
   - Verify port 4618 is available
   - Check NGINX configuration syntax
   - Ensure proper directory permissions

3. **Web interface not accessible**
   - Verify HT-MCP is running with `--bind-address 0.0.0.0:3618`
   - Check that `enableWebServer: true` was used when creating sessions
   - Ensure firewall isn't blocking ports 3618/4618

### Debug Mode

Enable debug output for detailed logging:
```bash
./run-with-ht-mcp.sh -a -d
```

This provides:
- HT-MCP server debug output
- NGINX proxy logs
- MCP server validation logs
- Container startup diagnostics

## File Structure

```
claude-task/
├── docker/                           # Docker configuration
│   ├── Dockerfile                   # Multi-stage container build
│   ├── docker-bake.hcl             # Docker buildx configuration
│   ├── entrypoint.sh               # Container initialization
│   └── nginx/
│       └── ht-mcp-proxy.conf       # NGINX WebSocket proxy
├── scripts/                         # Execution scripts
│   ├── run-with-ht-mcp.sh         # Main runner script
│   ├── test-ht-mcp.sh             # Setup and instructions
│   ├── test-docker.sh             # Docker testing
│   └── default-ht-mcp-prompt.txt  # Default comprehensive test prompt
├── docs/                           # Documentation
│   └── HT_MCP_INTEGRATION.md       # This file
├── examples/                       # Examples and testing
│   └── local-nginx-test/          # Local testing setup
└── modules/ht-mcp/                 # HT-MCP submodule
```

## Development

### Adding New Features

1. **New MCP Tools**: Add to `modules/ht-mcp/src/mcp/tools.rs`
2. **Container Changes**: Update `docker/Dockerfile`
3. **Proxy Configuration**: Modify `docker/nginx/ht-mcp-proxy.conf`
4. **Scripts**: Update `scripts/run-with-ht-mcp.sh`

### Testing Changes

1. Build and test locally:
   ```bash
   scripts/test-ht-mcp.sh
   ```

2. Test Docker integration:
   ```bash
   scripts/test-docker.sh
   ```

3. Test proxy configuration:
   ```bash
   cd examples/local-nginx-test
   ./start-nginx.sh
   ```

## Future Enhancements

### Planned Features

1. **Traefik Integration**: Support multiple task web views simultaneously
2. **Session Persistence**: Maintain terminal sessions across container restarts
3. **Enhanced Monitoring**: Additional metrics and logging
4. **Authentication**: User authentication for web interface access

### Configuration Options

Future versions will support:
- Custom port ranges
- SSL/TLS termination
- Load balancing for multiple HT-MCP instances
- Integration with external monitoring systems

## Contributing

When contributing to HT-MCP integration:

1. Follow the existing project structure
2. Update documentation for any new features
3. Test both local and Docker environments
4. Ensure backward compatibility with existing scripts

## References

- [HT-MCP Repository](https://github.com/cripplet/ht-mcp)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Claude Code Documentation](https://docs.anthropic.com/claude-code)