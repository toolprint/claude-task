group "default" {
  targets = ["claude-task-linux"]
}

target "claude-task-linux" {
  context = "."
  dockerfile = "Dockerfile"
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