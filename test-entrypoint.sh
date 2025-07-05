#!/bin/bash
# Test if the entrypoint script is working

echo "Testing Docker entrypoint with HT-MCP..."
echo ""

./target/release/claude-task run --build \
    --approval-tool-permission mcp__cco__approval_prompt \
    --ht-mcp-port 3618 \
    --debug \
    "echo 'Entrypoint test - this should show the entrypoint script output'" \
    --yes