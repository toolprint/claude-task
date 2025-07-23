// Claude Task Dagger CI Pipeline
package main

import (
	"context"
	"fmt"
	"strings"
)

type ClaudeTask struct{}

// Build targets for cross-compilation
type BuildTarget struct {
	OS   string
	Arch string
}

// Docker build configuration
type DockerConfig struct {
	Registry   string
	Image      string
	Tag        string
	GithubOrg  string
	Version    string
	DockerTag  string
	Push       bool
}

// getAllBuildTargets returns all supported build targets for release
func (m *ClaudeTask) getAllBuildTargets() []BuildTarget {
	return []BuildTarget{
		{"linux", "amd64"},
		{"linux", "arm64"},
		{"darwin", "amd64"},
		{"darwin", "arm64"},
		{"windows", "amd64"},
	}
}

// getCIBuildTargets returns limited build targets for CI (faster builds)
func (m *ClaudeTask) getCIBuildTargets() []BuildTarget {
	return []BuildTarget{
		{"linux", "amd64"},
		{"darwin", "amd64"},
	}
}

// GetRustContainer returns a container with Rust toolchain and cross-compilation support
func (m *ClaudeTask) GetRustContainer(ctx context.Context, source *Directory) *Container {
	return dag.Container().
		From("rust:1.75-slim").
		WithExec([]string{"apt-get", "update"}).
		WithExec([]string{"apt-get", "install", "-y", 
			"build-essential", 
			"pkg-config", 
			"libssl-dev",
			"musl-tools",
			"gcc-mingw-w64",
			"curl"}).
		// Install cross-compilation targets
		WithExec([]string{"rustup", "target", "add", 
			"x86_64-unknown-linux-musl",
			"aarch64-unknown-linux-musl", 
			"x86_64-pc-windows-gnu",
			"x86_64-apple-darwin",
			"aarch64-apple-darwin"}).
		// Install cross tool for easier cross-compilation
		WithExec([]string{"cargo", "install", "cross", "--git", "https://github.com/cross-rs/cross"}).
		WithWorkdir("/src").
		WithDirectory("/src", source)
}

// Test runs the test suite
func (m *ClaudeTask) Test(ctx context.Context, source *Directory) *Container {
	return m.GetRustContainer(ctx, source).
		WithExec([]string{"cargo", "test", "--", "--skip", "mcp"})
}

// Lint runs code formatting and linting checks
func (m *ClaudeTask) Lint(ctx context.Context, source *Directory) *Container {
	return m.GetRustContainer(ctx, source).
		WithExec([]string{"cargo", "fmt", "--check"}).
		WithExec([]string{"cargo", "clippy", "--", "-D", "warnings"})
}

// BuildBinary builds a single binary for the specified target
func (m *ClaudeTask) BuildBinary(ctx context.Context, source *Directory, os string, arch string) *File {
	container := m.GetRustContainer(ctx, source)
	
	var target string
	var buildCmd []string
	
	switch fmt.Sprintf("%s-%s", os, arch) {
	case "linux-amd64":
		target = "x86_64-unknown-linux-musl"
		buildCmd = []string{"cargo", "build", "--release", "--target", target}
	case "linux-arm64":
		target = "aarch64-unknown-linux-musl"
		buildCmd = []string{"cargo", "build", "--release", "--target", target}
	case "darwin-amd64":
		target = "x86_64-apple-darwin"
		buildCmd = []string{"cargo", "build", "--release", "--target", target}
	case "darwin-arm64":
		target = "aarch64-apple-darwin"
		buildCmd = []string{"cargo", "build", "--release", "--target", target}
	case "windows-amd64":
		target = "x86_64-pc-windows-gnu"
		buildCmd = []string{"cargo", "build", "--release", "--target", target}
	default:
		panic(fmt.Sprintf("Unsupported target: %s-%s", os, arch))
	}
	
	// Set environment variables for cross-compilation
	container = container.
		WithEnvVariable("CC_x86_64_unknown_linux_musl", "musl-gcc").
		WithEnvVariable("CC_aarch64_unknown_linux_musl", "aarch64-linux-musl-gcc").
		WithEnvVariable("CC_x86_64_pc_windows_gnu", "x86_64-w64-mingw32-gcc").
		WithEnvVariable("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER", "x86_64-w64-mingw32-gcc")
	
	container = container.WithExec(buildCmd)
	
	binaryName := "claude-task"
	if os == "windows" {
		binaryName += ".exe"
	}
	
	return container.File(fmt.Sprintf("/src/target/%s/release/%s", target, binaryName))
}

// BuildAll builds binaries for all supported platforms (used in releases)
func (m *ClaudeTask) BuildAll(ctx context.Context, source *Directory) *Directory {
	dir := dag.Directory()
	
	for _, target := range m.getAllBuildTargets() {
		binary := m.BuildBinary(ctx, source, target.OS, target.Arch)
		
		fileName := fmt.Sprintf("claude-task-%s-%s", target.OS, target.Arch)
		if target.OS == "windows" {
			fileName += ".exe"
		}
		
		dir = dir.WithFile(fileName, binary)
	}
	
	return dir
}

// BuildCI builds binaries for CI (limited targets for speed)
func (m *ClaudeTask) BuildCI(ctx context.Context, source *Directory) *Directory {
	dir := dag.Directory()
	
	for _, target := range m.getCIBuildTargets() {
		binary := m.BuildBinary(ctx, source, target.OS, target.Arch)
		
		fileName := fmt.Sprintf("claude-task-%s-%s", target.OS, target.Arch)
		if target.OS == "windows" {
			fileName += ".exe"
		}
		
		dir = dir.WithFile(fileName, binary)
	}
	
	return dir
}

// BuildDocker builds and optionally pushes Docker images
func (m *ClaudeTask) BuildDocker(ctx context.Context, source *Directory, config DockerConfig) *Container {
	// First, build the Linux binary (we need this for the Docker image)
	linuxBinary := m.BuildBinary(ctx, source, "linux", "amd64")
	
	// Create a minimal container with the binary
	container := dag.Container().
		From("debian:bookworm-slim").
		WithExec([]string{"apt-get", "update"}).
		WithExec([]string{"apt-get", "install", "-y", "ca-certificates", "git", "curl"}).
		WithExec([]string{"rm", "-rf", "/var/lib/apt/lists/*"}).
		WithFile("/usr/local/bin/claude-task", linuxBinary).
		WithExec([]string{"chmod", "+x", "/usr/local/bin/claude-task"}).
		WithEntrypoint([]string{"/usr/local/bin/claude-task"})
	
	// Add labels
	container = container.
		WithLabel("org.opencontainers.image.title", "claude-task").
		WithLabel("org.opencontainers.image.description", "Claude Task CLI tool").
		WithLabel("org.opencontainers.image.version", config.Version).
		WithLabel("org.opencontainers.image.source", fmt.Sprintf("https://github.com/%s/claude-task", config.GithubOrg))
	
	// Build image tags
	tags := []string{
		fmt.Sprintf("%s/%s:latest", config.Registry, config.Image),
		fmt.Sprintf("%s/%s:v%s", config.Registry, config.Image, config.Version),
		fmt.Sprintf("%s/%s:%s", config.Registry, config.Image, config.DockerTag),
	}
	
	if config.Push {
		// Push to registry
		for _, tag := range tags {
			container = container.WithRegistryAuth(config.Registry, "github", dag.SetSecret("github-token", config.GithubOrg))
			_, err := container.Publish(ctx, tag)
			if err != nil {
				panic(fmt.Sprintf("Failed to push %s: %v", tag, err))
			}
		}
	}
	
	return container
}

// CI runs the complete CI pipeline
func (m *ClaudeTask) CI(ctx context.Context, source *Directory) *Directory {
	// Run tests and linting in parallel
	testResult := m.Test(ctx, source)
	lintResult := m.Lint(ctx, source)
	
	// Wait for both to complete
	_, err := testResult.Sync(ctx)
	if err != nil {
		panic(fmt.Sprintf("Tests failed: %v", err))
	}
	
	_, err = lintResult.Sync(ctx)
	if err != nil {
		panic(fmt.Sprintf("Linting failed: %v", err))
	}
	
	// If tests and linting pass, build CI binaries
	return m.BuildCI(ctx, source)
}

// Release runs the complete release pipeline
func (m *ClaudeTask) Release(ctx context.Context, source *Directory, version string, githubOrg string, dockerTag string, pushDocker bool) *Directory {
	// Run CI first
	_ = m.CI(ctx, source)
	
	// Build all binaries
	binaries := m.BuildAll(ctx, source)
	
	// Build and push Docker images if requested
	if pushDocker {
		config := DockerConfig{
			Registry:  "ghcr.io",
			Image:     fmt.Sprintf("%s/claude-task", strings.ToLower(githubOrg)),
			Tag:       dockerTag,
			GithubOrg: githubOrg,
			Version:   version,
			DockerTag: dockerTag,
			Push:      true,
		}
		
		m.BuildDocker(ctx, source, config)
	}
	
	return binaries
}

// GetVersion extracts version from Cargo.toml
func (m *ClaudeTask) GetVersion(ctx context.Context, source *Directory) (string, error) {
	cargoToml, err := source.File("Cargo.toml").Contents(ctx)
	if err != nil {
		return "", err
	}
	
	// Simple parsing - look for version = "x.y.z"
	lines := strings.Split(cargoToml, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "version = ") {
			// Extract version from line like: version = "0.1.0"
			parts := strings.Split(line, "\"")
			if len(parts) >= 2 {
				return parts[1], nil
			}
		}
	}
	
	return "0.1.0", nil // fallback
}