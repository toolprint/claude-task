#!/bin/bash
# Docker entrypoint script for claude-task

set -e

# Configuration defaults (can be overridden via environment variables)
CCO_MCP_URL="${CCO_MCP_URL:-http://host.docker.internal:8660/mcp}"

# Output to stderr for visibility in container logs

if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Docker Entrypoint Script Starting" >&2
    echo "[DEBUG] Running as user: $(whoami)" >&2
    echo "[DEBUG] Working directory: $(pwd)" >&2
    echo "[DEBUG] Command to execute: $*" >&2
fi

# Copy base files to user home
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Setting up environment..." >&2
fi
cp -r /home/base/. /home/node/ 2>/dev/null || true

# Configure MCP servers
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Configuring MCP servers..." >&2
    echo "[DEBUG] Adding CCO approval server..." >&2
fi
claude mcp add-json -s user cco "{\"type\":\"http\",\"url\":\"$CCO_MCP_URL\"}" >/dev/null 2>&1 || {
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Failed to add CCO server" >&2
    fi
}

# Validate MCP configuration
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Validating MCP configuration..." >&2
    claude mcp list >&2 || echo "[DEBUG] Failed to list MCP servers" >&2
fi

# Add output marker for Claude output detection
echo "=== CLAUDE_OUTPUT_START ===" >&2

# Execute the command passed to the container
exec "$@"