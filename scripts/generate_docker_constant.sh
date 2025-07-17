#!/bin/bash

# Get the GitHub organization from environment variable or gh CLI
if [ -n "$CLAUDE_TASK_DOCKER_ORG" ]; then
    ORG="$CLAUDE_TASK_DOCKER_ORG"
else
    # Try to get the repository owner using gh CLI
    if command -v gh &> /dev/null; then
        ORG=$(gh repo view --json owner -q '.owner.login' 2>/dev/null | tr '[:upper:]' '[:lower:]')
    fi
    
    # Default fallback
    if [ -z "$ORG" ]; then
        ORG="onegrep"
    fi
fi

# Generate the Docker image name
DOCKER_IMAGE="ghcr.io/${ORG}/claude-task:latest"

# Create the constants file
cat > src/generated_constants.rs << EOF
/// Default Docker image name based on GitHub organization
pub const DEFAULT_DOCKER_IMAGE: &str = "${DOCKER_IMAGE}";
EOF

echo "Generated src/generated_constants.rs with Docker image: ${DOCKER_IMAGE}"