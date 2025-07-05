#!/bin/bash
# Start MCP Inspector with HT-MCP for testing

cd "$(dirname "$0")"

echo "🔍 Starting MCP Inspector with HT-MCP..."

# Check if ht-mcp binary exists
HT_MCP_PATH="../modules/ht-mcp/target/release/ht-mcp"
if [[ ! -f "$HT_MCP_PATH" ]]; then
    echo "❌ HT-MCP binary not found at: $HT_MCP_PATH"
    echo "Please build HT-MCP first:"
    echo "   cd modules/ht-mcp && cargo build --release"
    exit 1
fi

# Check if npx is available
if ! command -v npx >/dev/null 2>&1; then
    echo "❌ npx not found. Please install Node.js first."
    exit 1
fi

echo "✅ Found HT-MCP binary: $HT_MCP_PATH"
echo ""
echo "🚀 Starting MCP Inspector..."
echo "   HT-MCP will use default bind address (127.0.0.1:3618)"
echo "   Direct access: http://localhost:3618"
echo "   Nginx proxy: http://localhost:4618"
echo ""
echo "📋 Instructions:"
echo "   1. MCP Inspector will open in your browser"
echo "   2. You'll see HT-MCP server connected"
echo "   3. Use the 'ht_create_session' tool with these parameters:"
echo "      {"
echo "        \"enableWebServer\": true"
echo "      }"
echo "   4. Test both: http://localhost:3618 (direct) and http://localhost:4618 (proxy)"
echo ""
echo "Press Ctrl+C to stop..."
echo ""

# Run MCP Inspector with HT-MCP using default settings
# HT-MCP will bind to 127.0.0.1:3618 by default
npx @modelcontextprotocol/inspector "$HT_MCP_PATH" --debug