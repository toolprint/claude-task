#!/bin/bash
# Direct Docker test to see if entrypoint runs

echo "Testing Docker container with simple command..."

# Run container directly with Docker to bypass claude-task
docker run --rm \
  -v claude-task-home:/home/base:ro \
  claude-task:dev \
  /bin/bash -c "echo 'Container is running' && ls -la /usr/local/bin/docker-entrypoint.sh"