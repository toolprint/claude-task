#!/bin/bash
# Test if entrypoint script actually executes

echo "Testing entrypoint script execution..."

# Run the entrypoint script directly
docker run --rm \
  -v claude-task-home:/home/base:ro \
  claude-task:dev \
  /usr/local/bin/claude-entrypoint.sh echo "TEST COMMAND"