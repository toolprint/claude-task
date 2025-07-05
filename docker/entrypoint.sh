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
echo "Setting up nginx proxy: 0.0.0.0:4618 -> 127.0.0.1:3618"

# Check if port 4618 is already in use
echo "Checking if port 4618 is available..."
if lsof -i :4618 >/dev/null 2>&1; then
    echo "⚠️  WARNING: Port 4618 is already in use!"
    echo "   Process using port 4618:"
    lsof -i :4618 2>&1 | grep -v "^COMMAND" | head -5
elif netstat -tulpn 2>/dev/null | grep -q :4618; then
    echo "⚠️  WARNING: Port 4618 is already in use!"
    echo "   Process using port 4618:"
    netstat -tulpn 2>&1 | grep :4618 | head -5
else
    echo "✓ Port 4618 is available"
fi

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

# Create a temp file to capture nginx startup output
NGINX_LOG="/tmp/nginx_startup.log"
nginx -c /etc/nginx/ht-mcp-proxy.conf -g 'daemon off;' > "$NGINX_LOG" 2>&1 &
NGINX_PID=$!

# Give nginx a moment to start
sleep 2

# Check if nginx is actually running
if kill -0 $NGINX_PID 2>/dev/null; then
    echo "✓ Nginx proxy started (PID: $NGINX_PID)"
    
    # Double-check that nginx is actually listening on port 4618
    sleep 1
    if lsof -i :4618 >/dev/null 2>&1; then
        echo "✓ Confirmed: nginx is listening on port 4618"
    else
        echo "⚠️  WARNING: Nginx process exists but port 4618 is not open!"
        echo "   Check nginx error logs for issues."
    fi
else
    echo "❌ Nginx failed to start!"
    echo "Nginx startup errors:"
    cat "$NGINX_LOG"
    echo ""
    
    # Check if port is already in use
    if grep -q "bind() to 0.0.0.0:4618 failed" "$NGINX_LOG"; then
        echo "❌ Port 4618 is already in use!"
        echo "   Another process is using this port."
        lsof -i :4618 2>/dev/null || netstat -tulpn | grep :4618 2>/dev/null || echo "   (Unable to identify process)"
    fi
    
    echo ""
    echo "❌ FATAL: Nginx proxy is required for HT-MCP web interface"
    echo "   The container cannot continue without nginx running."
    exit 1
fi

# Clean up temp file
rm -f "$NGINX_LOG"

echo -e "\n=== Ready to run command ==="
echo "Note: HT-MCP will be started automatically by Claude when needed"
echo ""
echo "⚠️  IMPORTANT: The CCO approval server may override enableWebServer settings!"
echo "   If you see 'enableWebServer: false' in the logs, the web interface won't start."
echo "   To ensure web interface works, explicitly set enableWebServer: true in your prompt."
echo ""
echo "Web interface ports (when enableWebServer is true):"
echo "   - HT-MCP direct: http://localhost:3618 (container port)"
echo "   - Nginx proxy:   http://localhost:4618 (recommended, with CORS support)"
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