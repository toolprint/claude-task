#!/bin/bash
# Test entrypoint script directly without Claude

echo "Testing entrypoint script execution directly..."
echo ""

# Run a simple echo command through the entrypoint
./target/release/claude-task run \
    --ht-mcp-port 3618 \
    --debug \
    --skip-permissions \
    "/usr/local/bin/docker-entrypoint.sh echo 'ENTRYPOINT TEST SUCCESS'" \
    --yes