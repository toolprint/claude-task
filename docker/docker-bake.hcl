group "default" {
  targets = ["claude-task-linux"]
}

# GitHub Container Registry targets
group "ghcr" {
  targets = ["claude-task-ghcr"]
}

# Default target
target "claude-task-linux" {
  context = "."
  dockerfile = "docker/Dockerfile"
  tags = [
    "claude-task:latest",
    "claude-task:${DOCKER_TAG}"
  ]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=docker"]
}

variable "DOCKER_TAG" {
  default = "dev"
}

variable "VERSION" {
  default = "0.1.0"
}

variable "GITHUB_ORG" {
  default = "onegrep"
}

# GHCR target
target "claude-task-ghcr" {
  inherits = ["claude-task-linux"]
  tags = [
    "ghcr.io/${GITHUB_ORG}/claude-task:latest",
    "ghcr.io/${GITHUB_ORG}/claude-task:v${VERSION}",
    "ghcr.io/${GITHUB_ORG}/claude-task:${DOCKER_TAG}"
  ]
  platforms = ["linux/amd64", "linux/arm64"]
  output = ["type=registry"]
}