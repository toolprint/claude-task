# GitHub Actions Workflows

This directory contains GitHub Actions workflows that use Dagger for CI/CD automation.

## Workflows

### Dagger CI (`dagger-ci.yml`)

Runs on every push and pull request using Dagger for all CI operations.

- **Single Command**: Runs `dagger call ci` which executes:
  - Format checking with `cargo fmt`
  - Linting with `cargo clippy`
  - Tests on multiple platforms
  - Code coverage generation
- **Artifacts**: Coverage report uploaded as workflow artifact

### Dagger Release (`dagger-release.yml`)

Triggers on version tags (e.g., `v1.0.0`) to create GitHub releases with pre-built binaries.

- **Cross-Platform Builds**: Uses cargo-zigbuild for true cross-compilation from Linux
- **Supported Platforms**:
  - Linux: x86_64, aarch64
  - macOS: x86_64, aarch64, universal2 (fat binary)
  - Windows: x86_64
- **Parallel Builds**: All platforms built concurrently for speed
- **Artifacts**: Compressed .tar.gz archives with checksums
- **Docker Integration**: Builds and publishes Docker images to GHCR
- **Automatic Release**: Creates and publishes GitHub release with all artifacts

## Benefits of Dagger-based CI/CD

1. **Local Testing**: Run the exact same CI pipeline locally with `just dagger-ci`
2. **Reproducible Builds**: Containerized builds ensure consistency
3. **Better Caching**: Dagger automatically caches dependencies and build artifacts
4. **Simpler Workflows**: GitHub Actions files are minimal - just install Dagger and run
5. **Platform Agnostic**: The same Dagger module works with any CI system

## Local Testing

### Running CI Checks Locally

```bash
# Run complete CI pipeline
just dagger-ci

# Or use Dagger directly
dagger call ci --source .
```

### Building Releases Locally

```bash
# Build for a specific platform using cargo-zigbuild
dagger call zigbuild-single --source . --target x86_64-apple-darwin --version v1.0.0

# Build all releases for all platforms
dagger call release-zigbuild --source . --version v1.0.0 export --path ./release-artifacts/

# Build Docker image
dagger call build-docker --source . --github-org onegrep --version 1.0.0 --push false

# Run complete release pipeline (builds + Docker)
dagger call release --source . --version v1.0.0 --github-org onegrep --push-docker false
```

## Creating a Release

1. Update version in `Cargo.toml`
2. Commit changes
3. Create and push a tag:

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

4. The release workflow will automatically:
   - Build binaries for all platforms
   - Create a GitHub release
   - Upload all artifacts

## Troubleshooting

### Dagger Installation

If Dagger is not installed in CI:

```bash
curl -L https://dl.dagger.io/dagger/install.sh | sh
```

### Cross-Compilation with cargo-zigbuild

- **All platforms built from Linux**: No need for platform-specific runners
- **True cross-compilation**: Uses Zig toolchain for reliable cross-compilation
- **macOS SDK included**: Full support for macOS targets including universal2 binaries
- **Windows support**: Cross-compiles Windows binaries using MinGW
- **Fast parallel builds**: All platforms built concurrently

## Docker Integration

The Dagger pipeline now includes Docker image building and publishing:

- **Multi-platform images**: Builds for linux/amd64 and linux/arm64
- **GHCR publishing**: Automatically publishes to GitHub Container Registry
- **Flexible tagging**: Supports latest, version, and custom tags
- **Integrated workflow**: Docker builds are part of the release pipeline

### Docker Commands

```bash
# Build Docker image locally
dagger call build-docker --source . --github-org onegrep

# Build and push to registry
dagger call build-docker --source . --github-org onegrep --push true
```

The existing `docker-publish.yml` workflow remains for backward compatibility but the Dagger pipeline provides more flexibility and integration.

## Dependabot

Dependabot is configured to:

- Check for Rust dependency updates weekly
- Check for GitHub Actions updates weekly
- Group minor and patch updates together
- Create PRs with appropriate labels
