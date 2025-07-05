#!/bin/bash
# Run claude-task with HT-MCP and proper approval tool permission

# Default values
DEFAULT_PORT="3618"
DEFAULT_PROMPT="Please use the ht-mcp MCP server to create a new terminal session with enableWebServer set to true, then run 'ls -la'. The built-in Bash tool is restricted by permissions. IMPORTANT: You must set enableWebServer: true when creating the session."
DEBUG=""

# Check for debug flag
if [[ "$1" == "-d" ]] || [[ "$1" == "--debug" ]]; then
    DEBUG="--debug"
    shift  # Remove the debug flag from arguments
fi

# Parse remaining arguments
PORT="${1:-$DEFAULT_PORT}"
PROMPT="${2:-$DEFAULT_PROMPT}"

echo "🚀 Running claude-task with HT-MCP enabled (with CCO approval tool)"
echo "   Port: $PORT"
echo "   Prompt: $PROMPT"
echo ""
echo "📡 HT-MCP web interface will be available at: http://localhost:$PORT"
echo ""
echo "✅ Using CCO MCP approval tool for permission management"
echo "   Approval endpoint: https://auth-server-cco-mcp-873660917363.us-west1.run.app/mcp"
echo "   This ensures Claude must use HT-MCP instead of built-in tools"
echo ""

# Run with CCO MCP approval tool permission
# The permissions in settings.json will block built-in tools,
# forcing Claude to use HT-MCP
./target/release/claude-task run $DEBUG \
    --approval-tool-permission "mcp__cco__approval_prompt" \
    --ht-mcp-port "$PORT" \
    "$PROMPT"