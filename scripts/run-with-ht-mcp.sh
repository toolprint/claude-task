#!/bin/bash
# Run claude-task with HT-MCP enabled

# Default values
DEFAULT_PORT="3618"
# Load default prompt from external file
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/default-ht-mcp-prompt.txt" ]; then
    DEFAULT_PROMPT=$(cat "$SCRIPT_DIR/default-ht-mcp-prompt.txt")
else
    DEFAULT_PROMPT="Use the ht-mcp MCP server to create a terminal session with enableWebServer set to true, then list files and create a simple demo."
fi
DEBUG=""
APPROVAL=""

# Show usage if help is requested
if [[ "$1" == "-h" ]] || [[ "$1" == "--help" ]]; then
    echo "Usage: $0 [OPTIONS] [PORT] [PROMPT]"
    echo ""
    echo "Run claude-task with HT-MCP web interface enabled"
    echo ""
    echo "Options:"
    echo "  -d, --debug      Enable debug output"
    echo "  -a, --approval   Use CCO MCP approval tool (recommended)"
    echo ""
    echo "Arguments:"
    echo "  PORT    Port to expose HT-MCP web interface (default: $DEFAULT_PORT)"
    echo "  PROMPT  Task prompt for Claude (default: loaded from default-ht-mcp-prompt.txt)"
    echo ""
    echo "Default Prompt:"
    echo "  The default prompt includes a comprehensive development workflow scenario"
    echo "  that showcases various HT-MCP capabilities. See default-ht-mcp-prompt.txt"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Use defaults (no approval)"
    echo "  $0 -a                                # With CCO approval (recommended)"
    echo "  $0 -d -a                            # Debug mode with approval"
    echo "  $0 8080                              # Use port 8080 with default prompt"
    echo "  $0 -d 3618 'Create a hello world file'  # Debug mode with custom prompt"
    echo ""
    echo "Note: Using -a (approval) is recommended to ensure Claude uses HT-MCP"
    exit 0
fi

# Parse flags
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--debug)
            DEBUG="--debug"
            shift
            ;;
        -a|--approval)
            APPROVAL="--approval-tool-permission mcp__cco__approval_prompt"
            shift
            ;;
        -*)
            echo "Unknown option: $1"
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

# Parse remaining arguments
PORT="${1:-$DEFAULT_PORT}"
PROMPT="${2:-$DEFAULT_PROMPT}"

# Check if claude-task binary exists
if [[ ! -f "../target/release/claude-task" ]]; then
    echo "Error: claude-task binary not found at ../target/release/claude-task"
    echo "Please run 'just build-release' or './test-ht-mcp.sh' first to build the project"
    exit 1
fi

echo "🚀 Running claude-task with HT-MCP enabled"
echo "   Port: $PORT"
echo "   Prompt: $PROMPT"
if [[ -n "$APPROVAL" ]]; then
    echo "   Approval: CCO MCP (mcp__cco__approval_prompt)"
else
    echo "   Approval: None (⚠️  WARNING: Will skip permissions)"
fi
echo ""
echo "📡 HT-MCP web interface will be available at: http://localhost:$PORT"
echo ""

if [[ -n "$APPROVAL" ]]; then
    echo "✅ Using CCO MCP approval tool - Claude will be forced to use HT-MCP"
else
    echo "⚠️  WARNING: Without approval tool, Claude can bypass HT-MCP and use built-in tools"
    echo "   Consider using -a flag to enable approval tool"
fi
echo ""

# Run claude-task with HT-MCP
if [[ -n "$DEBUG" ]]; then
    echo "🔍 Running in debug mode..."
    echo ""
fi

../target/release/claude-task run ${DEBUG} ${APPROVAL} "$PROMPT"