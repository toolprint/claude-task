#!/bin/bash
# Test with simplified nginx config

cd "$(dirname "$0")"

echo "🧪 Testing with simplified nginx configuration..."

# Create temp directories in /tmp
sudo mkdir -p /tmp/nginx_client_temp /tmp/nginx_proxy_temp /tmp/nginx_fastcgi_temp /tmp/nginx_uwsgi_temp /tmp/nginx_scgi_temp
sudo chmod 777 /tmp/nginx_*_temp

# Kill any existing nginx
if [ -f nginx.pid ]; then
    nginx -c "$(pwd)/nginx-simple.conf" -s quit 2>/dev/null || true
    rm -f nginx.pid
fi

echo "🚀 Starting simplified nginx..."
nginx -c "$(pwd)/nginx-simple.conf" -g 'daemon off;' &
NGINX_PID=$!

echo "✅ Nginx started (PID: $NGINX_PID)"
echo ""
echo "🧪 Testing nginx functionality..."
sleep 1

# Test nginx is responding
echo "Testing nginx test endpoint:"
curl -s http://localhost:4618/test || echo "❌ Test endpoint failed"
echo ""

echo "📋 If test endpoint works, nginx is running correctly."
echo "📋 Now test the proxy to HT-MCP (make sure port 3618 is running first):"
echo "   curl http://localhost:4618/"
echo ""
echo "Press Ctrl+C to stop nginx..."

# Cleanup on exit
cleanup() {
    echo ""
    echo "🛑 Stopping nginx..."
    kill $NGINX_PID 2>/dev/null || true
    rm -f nginx.pid
}
trap cleanup SIGTERM SIGINT

wait $NGINX_PID