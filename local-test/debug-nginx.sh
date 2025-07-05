#!/bin/bash
# Debug nginx configuration issues

cd "$(dirname "$0")"

echo "🔍 Debugging nginx configuration..."
echo ""

# Check if nginx is running
echo "1. Checking if nginx is running on port 4618..."
if lsof -i :4618 >/dev/null 2>&1; then
    echo "✅ Something is listening on port 4618"
    lsof -i :4618
else
    echo "❌ Nothing listening on port 4618"
fi
echo ""

# Check if HT-MCP is running on 3618
echo "2. Checking if HT-MCP is running on port 3618..."
if lsof -i :3618 >/dev/null 2>&1; then
    echo "✅ Something is listening on port 3618"
    lsof -i :3618
else
    echo "❌ Nothing listening on port 3618 - start MCP Inspector first!"
fi
echo ""

# Test basic nginx functionality
echo "3. Testing nginx configuration..."
if nginx -c "$(pwd)/nginx-local.conf" -t; then
    echo "✅ Nginx configuration is valid"
else
    echo "❌ Nginx configuration has errors"
fi
echo ""

# Check nginx logs
echo "4. Checking nginx logs..."
if [ -f nginx-error.log ]; then
    echo "📋 Recent nginx errors:"
    tail -10 nginx-error.log
else
    echo "📋 No nginx error log found"
fi
echo ""

if [ -f nginx-access.log ]; then
    echo "📋 Recent nginx access log:"
    tail -10 nginx-access.log
else
    echo "📋 No nginx access log found"
fi
echo ""

# Test connectivity
echo "5. Testing connectivity..."
echo "Testing direct connection to HT-MCP (port 3618):"
curl -s -o /dev/null -w "HTTP %{http_code} - %{time_total}s\n" http://localhost:3618/ || echo "❌ Direct connection failed"

echo "Testing nginx proxy connection (port 4618):"
curl -s -o /dev/null -w "HTTP %{http_code} - %{time_total}s\n" http://localhost:4618/ || echo "❌ Proxy connection failed"
echo ""

echo "🎯 Next steps:"
echo "   - If nothing on 3618: Start './start-mcp-inspector.sh' first"
echo "   - If nothing on 4618: Start './start-nginx.sh' first"
echo "   - Check nginx logs above for specific errors"