# Start from a Node.js base image
FROM --platform=linux/amd64 node:22-slim AS build-linux-amd64
FROM --platform=linux/arm64 arm64v8/node:22-slim AS build-linux-arm64

ARG TZ
ENV TZ="$TZ"

# AMD64 stage
FROM build-linux-amd64 AS base-amd64
ARG TARGETARCH=amd64
ARG INCLUDE_HT_MCP=false

# Always copy the ht-mcp-release directory (contains .gitkeep when empty)
COPY docker/ht-mcp-release/ /tmp/ht-mcp-source/

# Install ht-mcp if requested and binary is available
RUN if [ "$INCLUDE_HT_MCP" = "true" ]; then \
      if [ -f "/tmp/ht-mcp-source/latest/ht-mcp-linux-x86_64" ]; then \
        echo "Installing ht-mcp for AMD64..." && \
        cp /tmp/ht-mcp-source/latest/ht-mcp-linux-x86_64 /usr/local/bin/ht-mcp && \
        chmod +x /usr/local/bin/ht-mcp && \
        file /usr/local/bin/ht-mcp && \
        (ldd /usr/local/bin/ht-mcp || echo "Binary is statically linked or incompatible") && \
        echo "ht-mcp binary installed successfully for AMD64"; \
      else \
        echo "ERROR: INCLUDE_HT_MCP=true but ht-mcp binary not found!"; \
        echo "Please run 'just prepare-docker-with-ht-mcp' first"; \
        exit 1; \
      fi; \
    else \
      echo "Skipping ht-mcp installation (INCLUDE_HT_MCP=$INCLUDE_HT_MCP)"; \
    fi && \
    rm -rf /tmp/ht-mcp-source

# ARM64 stage  
FROM build-linux-arm64 AS base-arm64
ARG TARGETARCH=arm64
ARG INCLUDE_HT_MCP=false

# Always copy the ht-mcp-release directory (contains .gitkeep when empty)
COPY docker/ht-mcp-release/ /tmp/ht-mcp-source/

# Install ht-mcp if requested and binary is available
RUN if [ "$INCLUDE_HT_MCP" = "true" ]; then \
      if [ -f "/tmp/ht-mcp-source/latest/ht-mcp-linux-aarch64" ]; then \
        echo "Installing ht-mcp for ARM64..." && \
        cp /tmp/ht-mcp-source/latest/ht-mcp-linux-aarch64 /usr/local/bin/ht-mcp && \
        chmod +x /usr/local/bin/ht-mcp && \
        file /usr/local/bin/ht-mcp && \
        (ldd /usr/local/bin/ht-mcp || echo "Binary is statically linked or incompatible") && \
        echo "ht-mcp binary installed successfully for ARM64"; \
      else \
        echo "ERROR: INCLUDE_HT_MCP=true but ht-mcp binary not found!"; \
        echo "Please run 'just prepare-docker-with-ht-mcp' first"; \
        exit 1; \
      fi; \
    else \
      echo "Skipping ht-mcp installation (INCLUDE_HT_MCP=$INCLUDE_HT_MCP)"; \
    fi && \
    rm -rf /tmp/ht-mcp-source

# Final stage
FROM base-${TARGETARCH} AS base

# Set NPM global packages directory and path
ENV NPM_CONFIG_PREFIX=/usr/local/share/npm-global
ENV PATH=$PATH:/usr/local/share/npm-global/bin

# Install basic development tools and nginx
RUN apt update && apt install -y less \
    git \
    procps \
    sudo \
    fzf \
    zsh \
    man-db \
    unzip \
    gnupg2 \
    gh \
    iptables \
    ipset \
    iproute2 \
    dnsutils \
    aggregate \
    jq \
    ripgrep \
    curl \
    nginx \
    && rm -rf /var/lib/apt/lists/*

ARG USERNAME=node

# Ensure default node user has access to /usr/local/share
RUN mkdir -p /usr/local/share/npm-global && \
    chown -R node:node /usr/local/share

# Persist bash history.
RUN SNIPPET="export PROMPT_COMMAND='history -a' && export HISTFILE=/commandhistory/.bash_history" \
  && mkdir /commandhistory \
  && touch /commandhistory/.bash_history \
  && chown -R $USERNAME /commandhistory

# Set `DEVCONTAINER` environment variable to help with orientation
ENV DEVCONTAINER=true

# Copy the entrypoint script and nginx config (before switching to node user)
COPY docker/entrypoint.sh /usr/local/bin/claude-entrypoint.sh
COPY docker/nginx/ht-mcp-proxy.conf /etc/nginx/ht-mcp-proxy.conf
RUN chmod +x /usr/local/bin/claude-entrypoint.sh

# Create workspace and config directories and set permissions
RUN mkdir -p /workspace /home/node/.claude && \
  chown -R node:node /workspace /home/node/.claude

WORKDIR /workspace

# Switch to non-root user for npm global packages
USER node

# Set the default shell to zsh rather than sh
ENV SHELL=/bin/zsh

# Default powerline10k theme
RUN sh -c "$(wget -O- https://github.com/deluan/zsh-in-docker/releases/download/v1.2.0/zsh-in-docker.sh)" -- \
  -p git \
  -p fzf \
  -a "source /usr/share/doc/fzf/examples/key-bindings.zsh" \
  -a "source /usr/share/doc/fzf/examples/completion.zsh" \
  -a "export PROMPT_COMMAND='history -a' && export HISTFILE=/commandhistory/.bash_history" \
  -x

# Install additional dev tools
RUN npm install -g @ast-grep/cli

# Install Claude Code (.claude/* and .claude.json will be under /home/node)
RUN npm install -g @anthropic-ai/claude-code

# Set working directory
WORKDIR /workspace

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/claude-entrypoint.sh"]