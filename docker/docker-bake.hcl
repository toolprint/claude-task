group "default" {
  targets = ["claude-task-linux"]
}

group "with-ht-mcp" {
  targets = ["claude-task-linux-with-ht-mcp"]
}

# Default target - without ht-mcp (lightweight)
target "claude-task-linux" {
  context = "."
  dockerfile = "docker/Dockerfile"
  args = {
    INCLUDE_HT_MCP = "false"
  }
  tags = [
    "claude-task:latest",
    "claude-task:${DOCKER_TAG}"
  ]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=docker"]
}

# Target with ht-mcp included
target "claude-task-linux-with-ht-mcp" {
  context = "."
  dockerfile = "docker/Dockerfile"
  args = {
    INCLUDE_HT_MCP = "true"
  }
  tags = [
    "claude-task:latest-with-ht-mcp",
    "claude-task:${DOCKER_TAG}-with-ht-mcp"
  ]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=docker"]
}

variable "DOCKER_TAG" {
  default = "dev"
}