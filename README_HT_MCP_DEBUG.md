# HT-MCP Integration Debugging Guide

## Problem
The HT-MCP server doesn't seem to be accessible from the browser when running claude-task with `--ht-mcp-port`.

## Debug Steps

### 1. Build the Project
```bash
cargo build --release
```

### 2. Run Setup
Ensure the configuration files are created:
```bash
./target/release/claude-task setup
```

### 3. Rebuild Docker Image
Force a rebuild of the Docker image to include the latest changes:
```bash
./target/release/claude-task run --build --ht-mcp-port 3618 "test" --yes
```

### 4. Run with Proper Permissions
For HT-MCP to work correctly, use the approval tool permission:
```bash
./run-ht-mcp-with-approval.sh -d
```

This script:
- Uses `--approval-tool-permission` to enable proper permissions
- Ensures Claude cannot use built-in tools (Bash, Edit, etc.)
- Forces Claude to use the HT-MCP server instead

### 5. Alternative: Debug with Skipped Permissions
If you want to see what happens without permissions (not recommended):
```bash
./run-with-ht-mcp.sh -d 3618 "Please use the ht-mcp MCP server"
```

Note: This will show a warning that skipping permissions defeats the purpose of HT-MCP.

## What the Debug Output Shows

The enhanced logging will show:
1. **HT-MCP Binary Check**: Verifies if ht-mcp is found in the container
2. **Server Startup**: Shows the HT-MCP server PID and startup logs
3. **Process Check**: Verifies if the HT-MCP process is running
4. **MCP Configuration**: Shows the contents of `.mcp.json` that Claude sees
5. **Settings Configuration**: Shows the permissions settings
6. **HT-MCP Logs**: Displays any error messages from the HT-MCP server

## Common Issues to Check

1. **Binary Not Found**: If "which ht-mcp" fails, the binary isn't being copied correctly
2. **Server Crash**: Check the HT-MCP log output for startup errors
3. **Port Already in Use**: Ensure port 3618 isn't already occupied
4. **MCP Config Not Found**: Verify `.mcp.json` is being created and mounted
5. **Permissions Not Working**: Check if `settings.json` is properly denying built-in tools

## Manual Container Inspection

To manually inspect a running container:
```bash
# List running containers
docker ps

# Execute into the container
docker exec -it <container-id> /bin/bash

# Inside the container, check:
ls -la /usr/local/bin/ht-mcp
cat /home/node/.claude/.mcp.json
cat /home/node/.claude/settings.json
ps aux | grep ht-mcp
```

## Next Steps

Based on the debug output:
- If HT-MCP binary is missing: Check Dockerfile COPY paths
- If server won't start: Check HT-MCP logs for errors
- If config is missing: Check credentials.rs setup
- If Claude ignores MCP: Check if settings.json permissions are working