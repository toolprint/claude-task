#!/bin/bash
# Start nginx locally for testing HT-MCP proxy

cd "$(dirname "$0")"

echo "🔧 Setting up local nginx test environment..."

# Create required temp directories in /tmp
sudo mkdir -p /tmp/nginx_client_temp /tmp/nginx_proxy_temp /tmp/nginx_fastcgi_temp /tmp/nginx_uwsgi_temp /tmp/nginx_scgi_temp
sudo chmod 777 /tmp/nginx_*_temp 2>/dev/null || true

# Check if nginx is installed
if ! command -v nginx >/dev/null 2>&1; then
    echo "❌ nginx not found. Please install nginx first:"
    echo "   macOS: brew install nginx"
    echo "   Ubuntu: sudo apt install nginx"
    exit 1
fi

# Kill any existing nginx processes using our config
if [ -f nginx.pid ]; then
    echo "🛑 Stopping existing nginx process..."
    nginx -c "$(pwd)/nginx-local.conf" -s quit 2>/dev/null || true
    rm -f nginx.pid
fi

echo "🚀 Starting nginx proxy on port 4618..."
echo "   Proxying: localhost:4618 -> localhost:3618"
echo "   Config: $(pwd)/nginx-local.conf"
echo ""

# Start nginx with our config
nginx -c "$(pwd)/nginx-local.conf" -g 'daemon off;' &
NGINX_PID=$!

echo "✅ Nginx started (PID: $NGINX_PID)"
echo "📍 Nginx will proxy http://localhost:4618 to http://localhost:3618"
echo ""
echo "🎯 Next steps:"
echo "   1. Start Claude Code or another MCP client with HT-MCP configured in another terminal"
echo "   2. Use ht_create_session with enableWebServer: true to create a session"
echo "   3. Test direct: http://localhost:3618 and proxy: http://localhost:4618"
echo ""
echo "📋 Logs:"
echo "   Access: $(pwd)/nginx-access.log"
echo "   Error:  $(pwd)/nginx-error.log"
echo ""
echo "Press Ctrl+C to stop nginx..."

# Handle cleanup
cleanup() {
    echo ""
    echo "🛑 Stopping nginx..."
    kill $NGINX_PID 2>/dev/null || true
    rm -f nginx.pid
    echo "✅ Cleanup complete"
}
trap cleanup SIGTERM SIGINT

# Wait for nginx process
wait $NGINX_PID