#!/bin/bash
# Docker entrypoint script for claude-task with HT-MCP integration

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

# Check HT-MCP binary availability
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Checking HT-MCP binary availability..." >&2
fi
HT_MCP_AVAILABLE=false

if command -v ht-mcp >/dev/null 2>&1; then
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] HT-MCP found at: $(which ht-mcp)" >&2
    fi
    # TODO: Switch to --version when HT-MCP adds version support
    if ht-mcp -h >/dev/null 2>&1; then
        if [ "${DEBUG_MODE}" = "true" ]; then
            echo "[DEBUG] HT-MCP help accessible" >&2
        fi
        HT_MCP_AVAILABLE=true
    else
        if [ "${DEBUG_MODE}" = "true" ]; then
            echo "[DEBUG] HT-MCP help check failed" >&2
        fi
    fi
elif [ -f /usr/local/bin/ht-mcp ]; then
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Found binary at /usr/local/bin/ht-mcp" >&2
        ls -la /usr/local/bin/ht-mcp >&2
    fi
    # Try to run it directly
    # TODO: Switch to --version when HT-MCP adds version support
    if /usr/local/bin/ht-mcp -h >/dev/null 2>&1; then
        if [ "${DEBUG_MODE}" = "true" ]; then
            echo "[DEBUG] HT-MCP help accessible" >&2
        fi
        HT_MCP_AVAILABLE=true
    else
        if [ "${DEBUG_MODE}" = "true" ]; then
            echo "[DEBUG] Binary exists but won't execute" >&2
        fi
    fi
else
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] HT-MCP binary not found - this is optional" >&2
        echo "[DEBUG] HT-MCP provides enhanced terminal monitoring through a web interface" >&2
        echo "[DEBUG] Tasks can still run normally without it" >&2
    fi
fi

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

if [ "$HT_MCP_AVAILABLE" = "true" ]; then
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Adding HT-MCP server..." >&2
    fi
    claude mcp add-json -s user ht-mcp '{"command":"ht-mcp","args":["--debug"]}' >/dev/null 2>&1 || {
        if [ "${DEBUG_MODE}" = "true" ]; then
            echo "[DEBUG] Failed to add HT-MCP server" >&2
        fi
    }
else
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Skipping HT-MCP server registration - binary not available" >&2
    fi
fi

# Validate MCP configuration
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Validating MCP configuration..." >&2
    claude mcp list >&2 || echo "[DEBUG] Failed to list MCP servers" >&2
fi

if [ "$HT_MCP_AVAILABLE" = "true" ]; then
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Starting HT-MCP web proxy..." >&2
        echo "[DEBUG] Setting up nginx proxy: 0.0.0.0:4618 -> 127.0.0.1:3618" >&2
    fi
else
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Skipping HT-MCP web proxy setup" >&2
        echo "[DEBUG] HT-MCP not available - skipping nginx proxy setup" >&2
        echo "[DEBUG] Claude Code will use built-in tools instead of HT-MCP" >&2
    fi
    # Add output marker for Claude output detection
    echo "=== CLAUDE_OUTPUT_START ===" >&2
    exec "$@"
fi

# Check if nginx proxy port is already in use
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Checking if port 4618 is available..." >&2
    if lsof -i :4618 >/dev/null 2>&1; then
        echo "[DEBUG] WARNING: Port 4618 is already in use!" >&2
        echo "[DEBUG] Process using port 4618:" >&2
        lsof -i :4618 2>&1 | grep -v "^COMMAND" | head -5 >&2
    elif netstat -tulpn 2>/dev/null | grep -q :4618; then
        echo "[DEBUG] WARNING: Port 4618 is already in use!" >&2
        echo "[DEBUG] Process using port 4618:" >&2
        netstat -tulpn 2>&1 | grep :4618 | head -5 >&2
    else
        echo "[DEBUG] Port 4618 is available" >&2
    fi
fi

# Create nginx directories in node user's home
mkdir -p /home/node/nginx/client_temp /home/node/nginx/proxy_temp /home/node/nginx/fastcgi_temp /home/node/nginx/uwsgi_temp /home/node/nginx/scgi_temp

# Create fallback page for when HT-MCP is not running
cat > /home/node/nginx/ht-mcp-offline.html << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>HT-MCP Web Server - Offline</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .container {
            background: white;
            border-radius: 12px;
            padding: 2rem;
            box-shadow: 0 10px 30px rgba(0,0,0,0.2);
            max-width: 500px;
            text-align: center;
        }
        h1 {
            color: #333;
            margin-bottom: 1rem;
            font-size: 1.8rem;
        }
        .status {
            background: #fff3cd;
            border: 1px solid #ffeaa7;
            border-radius: 6px;
            padding: 1rem;
            margin: 1rem 0;
            color: #856404;
        }
        .info {
            background: #d1ecf1;
            border: 1px solid #bee5eb;
            border-radius: 6px;
            padding: 1rem;
            margin: 1rem 0;
            color: #0c5460;
            text-align: left;
        }
        .code {
            background: #f8f9fa;
            border: 1px solid #e9ecef;
            border-radius: 4px;
            padding: 0.5rem;
            font-family: 'Monaco', 'Menlo', monospace;
            font-size: 0.9rem;
            margin: 0.5rem 0;
        }
        .refresh-btn {
            background: #007bff;
            color: white;
            border: none;
            padding: 0.75rem 1.5rem;
            border-radius: 6px;
            cursor: pointer;
            font-size: 1rem;
            margin-top: 1rem;
        }
        .refresh-btn:hover {
            background: #0056b3;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🔧 HT-MCP Web Server</h1>
        
        <div class="status">
            <strong>Status:</strong> Not Running
        </div>
        
        <p>The HT-MCP (Headless Terminal MCP Server) web interface is not currently active.</p>
        
        <div class="info">
            <strong>To start the web interface:</strong>
            <ul>
                <li>Use Claude to create a terminal session</li>
                <li>Make sure to set <code class="code">enableWebServer: true</code></li>
                <li>The web interface will automatically become available</li>
            </ul>
        </div>
        
        <div class="info">
            <strong>Available ports:</strong>
            <ul>
                <li><strong>Direct HT-MCP:</strong> <code class="code">http://localhost:3618</code></li>
                <li><strong>NGINX Proxy:</strong> <code class="code">http://localhost:4618</code> (this page)</li>
            </ul>
        </div>
        
        <button class="refresh-btn" onclick="window.location.reload()">
            🔄 Refresh Page
        </button>
        
        <p style="margin-top: 2rem; font-size: 0.9rem; color: #666;">
            This page is served by NGINX while waiting for HT-MCP to start.
        </p>
    </div>
</body>
</html>
EOF

# Ensure node user owns the nginx directory
chown -R node:node /home/node/nginx

# Check if nginx config exists
if [ ! -f /etc/nginx/ht-mcp-proxy.conf ]; then
    echo "❌ Nginx config not found at /etc/nginx/ht-mcp-proxy.conf" >&2
    exit 1
fi

if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Using static nginx config: 0.0.0.0:4618 -> 127.0.0.1:3618" >&2
    
    # Debug: Test nginx config syntax
    echo "[DEBUG] Testing nginx configuration syntax:" >&2
    nginx -t -c /etc/nginx/ht-mcp-proxy.conf 2>&1 | sed 's/^/[DEBUG] /' >&2
fi

# Start nginx with custom config in background
if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Starting nginx..." >&2
fi

# Create a temp file to capture nginx startup output
NGINX_LOG="/tmp/nginx_startup.log"
nginx -c /etc/nginx/ht-mcp-proxy.conf -g 'daemon off;' > "$NGINX_LOG" 2>&1 &
NGINX_PID=$!

# Give nginx a moment to start
sleep 2

# Check if nginx is actually running
if kill -0 $NGINX_PID 2>/dev/null; then
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Nginx proxy started (PID: $NGINX_PID)" >&2
        
        # Double-check that nginx is actually listening on port 4618
        sleep 1
        if lsof -i :4618 >/dev/null 2>&1; then
            echo "[DEBUG] Confirmed: nginx is listening on port 4618" >&2
        else
            # Try alternative detection methods
            if netstat -tlnp 2>/dev/null | grep -q ":4618.*nginx" || ss -tlnp 2>/dev/null | grep -q ":4618.*nginx"; then
                echo "[DEBUG] Confirmed: nginx is listening on port 4618 (detected via netstat/ss)" >&2
            else
                echo "[DEBUG] WARNING: Cannot confirm nginx is listening on port 4618" >&2
                echo "[DEBUG] This may be a detection issue rather than a binding problem." >&2
                echo "[DEBUG] Recent nginx error log entries:" >&2
                tail -5 /home/node/nginx/error.log 2>&1 | sed 's/^/[DEBUG] /' >&2 || echo "[DEBUG] No error log found" >&2
                echo "[DEBUG] Continuing anyway - nginx appears to be running (PID: $NGINX_PID)" >&2
                echo "[DEBUG] If you can access http://localhost:4618, the proxy is working correctly." >&2
            fi
        fi
    fi
else
    echo "❌ Nginx failed to start!" >&2
    echo "Nginx startup errors:" >&2
    cat "$NGINX_LOG" >&2
    echo "" >&2
    
    # Check if port is already in use
    if grep -q "bind() to 0.0.0.0:4618 failed" "$NGINX_LOG"; then
        echo "❌ Port 4618 is already in use!" >&2
        echo "   Another process is using this port." >&2
        lsof -i :4618 2>/dev/null || netstat -tulpn | grep :4618 2>/dev/null || echo "   (Unable to identify process)" >&2
    fi
    
    echo "" >&2
    echo "❌ FATAL: Nginx proxy is required for HT-MCP web interface" >&2
    echo "   The container cannot continue without nginx running." >&2
    exit 1
fi

# Clean up temp file
rm -f "$NGINX_LOG"

if [ "${DEBUG_MODE}" = "true" ]; then
    echo "[DEBUG] Ready to run command" >&2
    echo "[DEBUG] Note: HT-MCP will be started automatically by Claude when needed" >&2
    echo "[DEBUG] Web interface ports (when enableWebServer is true):" >&2
    echo "[DEBUG]   - HT-MCP direct: http://localhost:3618 (container port)" >&2
    echo "[DEBUG]   - Nginx proxy:   http://localhost:4618 (recommended, with CORS support)" >&2
fi

# Function to cleanup background processes
cleanup() {
    if [ "${DEBUG_MODE}" = "true" ]; then
        echo "[DEBUG] Cleaning up background processes..." >&2
    fi
    kill $NGINX_PID 2>/dev/null || true
    exit
}
trap cleanup SIGTERM SIGINT

# Add output markers for Claude output detection
echo "=== CLAUDE_OUTPUT_START ===" >&2

# Execute the command passed to the container
exec "$@"