#!/bin/bash
# Quick test of entrypoint without rebuilding Docker image

echo "Testing entrypoint script execution..."
echo ""

./target/release/claude-task run \
    --approval-tool-permission mcp__cco__approval_prompt \
    --ht-mcp-port 3618 \
    --debug \
    "echo 'Testing if entrypoint runs'" \
    --yes