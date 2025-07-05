#!/bin/bash
# Debug what's actually in the container

echo "Debugging container contents..."
echo ""

./target/release/claude-task run \
    --approval-tool-permission mcp__cco__approval_prompt \
    --ht-mcp-port 3618 \
    --debug \
    "ls -la /usr/local/bin/ && echo '---' && cat /usr/local/bin/docker-entrypoint.sh || echo 'Entrypoint script not found'" \
    --yes