#!/bin/bash
# Run setup to create the configuration files

echo "Running claude-task setup to create configuration files..."
echo "This will create:"
echo "  - ~/.claude-task/home/.mcp.json (MCP server configuration)"
echo "  - ~/.claude-task/home/settings.json (Permission restrictions)"
echo ""

./target/release/claude-task setup

echo ""
echo "Checking if files were created..."
if [[ -f ~/.claude-task/home/.mcp.json ]]; then
    echo "✓ .mcp.json created"
    echo "Contents:"
    cat ~/.claude-task/home/.mcp.json
else
    echo "✗ .mcp.json not found"
fi

echo ""
if [[ -f ~/.claude-task/home/settings.json ]]; then
    echo "✓ settings.json created"
    echo "Contents:"
    cat ~/.claude-task/home/settings.json
else
    echo "✗ settings.json not found"
fi