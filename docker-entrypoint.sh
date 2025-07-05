#!/bin/bash
# Docker entrypoint script for claude-task with HT-MCP integration

set -e

# Output to stderr for visibility in container logs

echo "=== Docker Entrypoint Script Starting ===" >&2
echo "Running as user: $(whoami)" >&2
echo "Working directory: $(pwd)" >&2
echo "Command to execute: $*" >&2
echo "" >&2

# Copy base files to user home
echo "Setting up environment..." >&2
cp -r /home/base/. /home/node/ 2>/dev/null || true

# Validate HT-MCP binary
echo "=== Validating HT-MCP binary ===" >&2
if command -v ht-mcp >/dev/null 2>&1; then
    echo "✓ HT-MCP found at: $(which ht-mcp)"
    ht-mcp --version || echo "⚠️ HT-MCP version check failed"
else
    echo "✗ HT-MCP binary not found in PATH"
    echo "Checking /usr/local/bin/ht-mcp directly..."
    if [ -f /usr/local/bin/ht-mcp ]; then
        echo "Found binary at /usr/local/bin/ht-mcp"
        ls -la /usr/local/bin/ht-mcp
        # Try to run it directly
        /usr/local/bin/ht-mcp --version || echo "Binary exists but won't execute"
    else
        echo "Binary not found at /usr/local/bin/ht-mcp"
    fi
    exit 1
fi

# Configure MCP servers
echo -e "\n=== Configuring MCP servers ==="
echo "Adding CCO approval server..."
claude mcp add-json -s user cco '{"type":"http","url":"https://auth-server-cco-mcp-873660917363.us-west1.run.app/mcp"}' || echo "⚠️ Failed to add CCO server"

echo "Adding HT-MCP server..."
claude mcp add-json -s user ht-mcp '{"command":"ht-mcp","args":["--debug"]}' || echo "⚠️ Failed to add HT-MCP server"

# Validate MCP configuration
echo -e "\n=== Validating MCP configuration ==="
claude mcp list || echo "⚠️ Failed to list MCP servers"

echo -e "\n=== Starting HT-MCP web proxy ==="
echo "Setting up nginx proxy: 0.0.0.0:3618 -> 127.0.0.1:3618"

# Create nginx directories in node user's home
mkdir -p /home/node/nginx/client_temp /home/node/nginx/proxy_temp /home/node/nginx/fastcgi_temp /home/node/nginx/uwsgi_temp /home/node/nginx/scgi_temp
# Ensure node user owns the nginx directory
chown -R node:node /home/node/nginx

# Check if nginx config exists
if [ ! -f /etc/nginx/ht-mcp-proxy.conf ]; then
    echo "❌ Nginx config not found at /etc/nginx/ht-mcp-proxy.conf"
    exit 1
fi

# Start nginx with custom config in background
echo "Starting nginx..."
nginx -c /etc/nginx/ht-mcp-proxy.conf -g 'daemon off;' 2>&1 &
NGINX_PID=$!

# Give nginx a moment to start
sleep 1

# Check if nginx is actually running
if kill -0 $NGINX_PID 2>/dev/null; then
    echo "✓ Nginx proxy started (PID: $NGINX_PID)"
else
    echo "❌ Nginx failed to start!"
    echo "Trying to start nginx in foreground to see errors:"
    nginx -c /etc/nginx/ht-mcp-proxy.conf -g 'daemon off;' 2>&1
    echo ""
    echo "❌ FATAL: Nginx proxy is required for HT-MCP web interface"
    echo "   The container cannot continue without nginx running."
    exit 1
fi

echo -e "\n=== Ready to run command ==="
echo "Note: HT-MCP will be started automatically by Claude when needed"
echo "IMPORTANT: When creating sessions, use enableWebServer: true for port 3618"
echo "Web interface will be accessible via proxy on 0.0.0.0:3618"
echo ""

# Function to cleanup background processes
cleanup() {
    echo "Cleaning up background processes..."
    kill $NGINX_PID 2>/dev/null || true
    exit
}
trap cleanup SIGTERM SIGINT

# Execute the command passed to the container
exec "$@"