#!/usr/bin/env -S just --justfile

# Get GitHub organization from git remote or environment variable
github_org := env("CLAUDE_TASK_DOCKER_ORG", `git remote get-url origin 2>/dev/null | sed -E 's|.*github.com[:/]([^/]+)/.*|\1|' | tr '[:upper:]' '[:lower:]' || echo "onegrep"`)

# Docker image name based on GitHub organization
docker_image := "ghcr.io/" + github_org + "/claude-task"

_default:
    @just -l -u

# Brew installation
[group('setup')]
brew:
    brew update & brew bundle install --file=./Brewfile

# Rust Development Commands

# Build the project
[group('rust')]
build:
    @echo "🔨 Building claude-task..."
    @./scripts/generate_docker_constant.sh
    cargo build

# Build in release mode
[group('rust')]
build-release:
    @echo "🔨 Building claude-task (release)..."
    @./scripts/generate_docker_constant.sh
    cargo build --release
    @just release-info

# Install tq (TOML query tool) for better TOML parsing
[group('rust')]
install-tq:
    @echo "📦 Installing tq (TOML query tool)..."
    cargo install --git https://github.com/cryptaliagy/tomlq

# Show information about release binaries
[group('rust')]
release-info:
    #!/usr/bin/env bash
    echo "============================="
    echo "📦 Release Binary Information"
    echo "============================="
    echo ""
    
    if [ ! -d "target/release" ]; then
        echo "❌ Release directory not found"
        echo "   Run 'just build-release' first"
        exit 0
    fi
    
    echo "🗂️  Release Directory: target/release/"
    echo ""
    
    # Parse TOML to get binary names
    if command -v tq >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
        echo "🔍 Using tq + jq to parse Cargo.toml"
        binaries=$(tq -o json -f Cargo.toml 'bin' 2>/dev/null | jq -r '.[].name' 2>/dev/null | tr '\n' ' ')
    elif command -v tq >/dev/null 2>&1; then
        echo "🔍 Using tq to parse Cargo.toml (install jq for better parsing)"
        bin_json=$(tq -o json -f Cargo.toml 'bin' 2>/dev/null)
        # Extract names from JSON manually
        binaries=$(echo "$bin_json" | sed 's/.*"name":"\([^"]*\)".*/\1/g' | tr '\n' ' ')
    else
        echo "🔍 Using AWK to parse Cargo.toml (fallback - install tq for better parsing)"
        echo "   Install with: just install-tq"
        binaries=$(awk '
        /^\[\[bin\]\]/ { in_bin=1; next }
        /^\[/ { in_bin=0 }
        in_bin && /^name = / {
            gsub(/^name = "|"$/, "")
            print
        }
        ' Cargo.toml | tr '\n' ' ')
    fi
    
    if [ -z "$binaries" ]; then
        echo "❌ No [[bin]] sections found in Cargo.toml"
        echo "   Check Cargo.toml configuration"
        exit 0
    fi
    
    echo "🔍 Binaries defined in Cargo.toml: $binaries"
    echo ""
    
    found_any=false
    for binary in $binaries; do
        if [ -f "target/release/$binary" ]; then
            echo "🔧 Binary: $binary"
            echo "   📍 Path: target/release/$binary"
            echo "   📏 Size: $(du -h target/release/$binary | cut -f1)"
            echo "   🏗️  Platform: $(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"
            echo "   📅 Modified: $(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S' target/release/$binary 2>/dev/null || stat -c '%y' target/release/$binary 2>/dev/null | cut -d'.' -f1)"
            if command -v file >/dev/null 2>&1; then
                echo "   🔍 Type: $(file target/release/$binary | cut -d':' -f2 | sed 's/^ *//')"
            fi
            echo ""
            found_any=true
        else
            echo "❌ Binary $binary not found in target/release/"
            echo ""
        fi
    done
    
    if [ "$found_any" = false ]; then
        echo "❌ No binaries found in target/release/"
        echo "   Run 'just build-release' first"
    fi

# Install release binaries locally and show installation info
[group('rust')]
install: build-release
    #!/usr/bin/env bash
    echo "📦 Installing Release Binaries"
    echo "=============================="
    echo ""
    
    # Parse TOML to get binary names (same logic as release-info)
    if command -v tq >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
        echo "🔍 Using tq + jq to parse Cargo.toml"
        binaries=$(tq -o json -f Cargo.toml 'bin' 2>/dev/null | jq -r '.[].name' 2>/dev/null | tr '\n' ' ')
    elif command -v tq >/dev/null 2>&1; then
        echo "🔍 Using tq to parse Cargo.toml"
        bin_json=$(tq -o json -f Cargo.toml 'bin' 2>/dev/null)
        binaries=$(echo "$bin_json" | sed 's/.*"name":"\([^"]*\)".*/\1/g' | tr '\n' ' ')
    else
        echo "🔍 Using AWK to parse Cargo.toml"
        binaries=$(awk '
        /^\[\[bin\]\]/ { in_bin=1; next }
        /^\[/ { in_bin=0 }
        in_bin && /^name = / {
            gsub(/^name = "|"$/, "")
            print
        }
        ' Cargo.toml | tr '\n' ' ')
    fi
    
    if [ -z "$binaries" ]; then
        echo "❌ No [[bin]] sections found in Cargo.toml"
        exit 1
    fi
    
    echo "🔍 Installing binaries: $binaries"
    echo ""
    
    # Install using cargo install
    echo "🚀 Running: cargo install --path . --force"
    if cargo install --path . --force; then
        echo ""
        echo "✅ Installation completed successfully!"
        echo ""
        
        # Show installation information  
        if [ -n "$CARGO_HOME" ]; then
            cargo_bin_dir="$CARGO_HOME/bin"
        else
            cargo_bin_dir="$HOME/.cargo/bin"
        fi
        
        echo "📂 Installation Directory: $cargo_bin_dir"
        echo ""
        
        for binary in $binaries; do
            if [ -f "$cargo_bin_dir/$binary" ]; then
                echo "🔧 Binary: $binary"
                echo "   📍 Path: $cargo_bin_dir/$binary"
                echo "   📏 Size: $(du -h $cargo_bin_dir/$binary | cut -f1)"
                echo "   🏗️  Platform: $(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"
                echo "   📅 Installed: $(stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S' $cargo_bin_dir/$binary 2>/dev/null || stat -c '%y' $cargo_bin_dir/$binary 2>/dev/null | cut -d'.' -f1)"
                if command -v file >/dev/null 2>&1; then
                    echo "   🔍 Type: $(file $cargo_bin_dir/$binary | cut -d':' -f2 | sed 's/^ *//')"
                fi
                echo ""
            else
                echo "❌ Binary $binary not found at $cargo_bin_dir/$binary"
                echo ""
            fi
        done
        
        echo "💡 Usage:"
        echo "   Run directly: $binary --help"
        echo "   Or ensure ~/.cargo/bin is in your PATH"
        echo ""
        
        # Create symlink for ct -> claude-task
        if [ -f "$cargo_bin_dir/claude-task" ]; then
            echo "🔗 Creating symlink: ct -> claude-task"
            ln -sf "$cargo_bin_dir/claude-task" "$cargo_bin_dir/ct"
            if [ -f "$cargo_bin_dir/ct" ]; then
                echo "   ✅ Symlink created successfully: $cargo_bin_dir/ct"
            else
                echo "   ❌ Failed to create symlink"
            fi
        fi
        
    else
        echo ""
        echo "❌ Installation failed!"
        exit 1
    fi

# Run cli with arguments (example: just run --help)
[group('rust')]
run *args:
    @echo "🚀 Running cli with args: {{args}}"
    cargo run -- {{args}}

# Run tests
[group('rust')]
test:
    @echo "🧪 Running tests..."
    cargo test

# Run only MCP tests
[group('rust')]
test-mcp:
    @echo "🧪 Running MCP tests..."
    cargo test -- --ignored mcp

# Check code without building
[group('rust')]
check:
    @echo "🔍 Checking code..."
    cargo check

# Format code
[group('rust')]
fmt:
    @echo "🎨 Formatting code..."
    cargo fmt

# Run clippy linter
[group('rust')]
clippy:
    @echo "📎 Running clippy..."
    cargo clippy

# Clean build artifacts
[group('rust')]
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean

# Formatting Commands

# Check all formatting
[group('format')]
check-fmt:
    @echo "🔍 Checking Rust formatting..."
    cargo fmt --check

# Pre-commit validation - runs all checks required before committing
[group('format')]
pre-commit:
    #!/usr/bin/env bash
    echo "🔄 Running pre-commit validation..."
    echo "=================================="
    echo ""
    
    # Track success for checks and linters
    checks_success=true
    
    # 1. Static check (cargo check)
    echo "1️⃣  Static code check..."
    if cargo check; then
        echo "   ✅ Static check passed"
    else
        echo "   ❌ Static check failed"
        checks_success=false
    fi
    echo ""
    
    # 2. Code formatting check
    echo "2️⃣  Code formatting check..."
    if cargo fmt --check; then
        echo "   ✅ Code formatting is correct"
    else
        echo "   ❌ Code formatting issues found"
        echo "   💡 Run 'just fmt' to fix formatting"
        checks_success=false
    fi
    echo ""
    
    # 3. Clippy linter
    echo "3️⃣  Clippy linter check..."
    if cargo clippy -- -D warnings; then
        echo "   ✅ Clippy linter passed"
    else
        echo "   ❌ Clippy linter found issues"
        checks_success=false
    fi
    echo ""
    
    # Check if we should proceed to tests
    if [ "$checks_success" = false ]; then
        echo "=================================="
        echo "❌ FAILURE: Code checks and linters failed"
        echo "🔧 Please fix the above issues before running tests"
        echo "💡 Once fixed, run 'just pre-commit' again to include tests"
        exit 1
    fi
    
    # 4. Tests (only run if all checks passed, excluding MCP tests)
    echo "4️⃣  Running tests (excluding MCP tests)..."
    if cargo test -- --skip mcp; then
        echo "   ✅ All tests passed (MCP tests excluded from pre-commit)"
    else
        echo "   ❌ Some tests failed"
        echo ""
        echo "=================================="
        echo "❌ FAILURE: Tests failed"
        echo "🔧 Please fix the failing tests before committing"
        exit 1
    fi
    echo ""
    
    # Final success message
    echo "=================================="
    echo "🎉 SUCCESS: All pre-commit checks passed!"
    echo "✅ Code is ready for commit"

# =====================================
# Dagger CI/CD Commands
# =====================================

# Run Dagger CI pipeline locally
[group('dagger')]
dagger-ci:
    @echo "🚀 Running Dagger CI pipeline..."
    dagger call ci --source .

# Run CI and then build Docker locally
[group('dagger')]
ci-and-docker:
    @echo "🚀 Running CI pipeline..."
    @just dagger-ci
    @echo ""
    @echo "🐳 Building Docker images..."
    @just docker-bake
    @echo ""
    @echo "✅ CI and Docker build completed!"

# Run Dagger format check
[group('dagger')]
dagger-format:
    @echo "🔍 Checking code formatting with Dagger..."
    dagger call format --source .

# Run Dagger lint
[group('dagger')]
dagger-lint:
    @echo "📋 Running clippy with Dagger..."
    dagger call lint --source .

# Run Dagger tests
[group('dagger')]
dagger-test platform="linux/amd64":
    @echo "🧪 Running tests on {{ platform }} with Dagger..."
    dagger call test --source . --platform {{ platform }}

# Run Dagger coverage
[group('dagger')]
dagger-coverage:
    @echo "📊 Generating coverage report with Dagger..."
    dagger call coverage --source . export --path ./tarpaulin-report.html
    @echo "✅ Coverage report saved to tarpaulin-report.html"

# Build with Dagger
[group('dagger')]
dagger-build platform="linux/amd64":
    @echo "🔨 Building for {{ platform }} with Dagger..."
    @mkdir -p ./build
    dagger call build --source . --platform {{ platform }} export --path ./build/claude-task-debug-{{ replace(platform, "/", "-") }}

# Build release with Dagger
[group('dagger')]
dagger-build-release platform="linux/amd64":
    @echo "📦 Building release for {{ platform }} with Dagger..."
    @mkdir -p ./build
    dagger call build-release --source . --platform {{ platform }} export --path ./build/claude-task-release-{{ replace(platform, "/", "-") }}

# Build releases for all platforms using Dagger with zigbuild (parallel execution)
[group('dagger')]
dagger-release version="v0.1.0":
    @echo "🚀 Building all platform releases in parallel with Dagger + zigbuild..."
    @mkdir -p ./release-artifacts
    dagger call release-zigbuild --source . --version {{ version }} export --path ./release-artifacts/
    @echo "✅ All platform releases built successfully!"
    @echo "📦 Release artifacts:"
    @ls -la ./release-artifacts/

# Build Docker image using Dagger
[group('dagger')]
dagger-docker registry="ghcr.io" version="v0.1.0" docker-tag="dev" push="false":
    @echo "🐳 Building Docker image with Dagger..."
    dagger call build-docker --source . --registry {{ registry }} --github-org {{ github_org }} --version {{ version }} --docker-tag {{ docker-tag }} --push {{ push }}

# Run complete release pipeline (binaries + Docker) using Dagger
[group('dagger')]
dagger-release-all version="v0.1.0" docker-tag="release" push-docker="false":
    @echo "🚀 Running complete release pipeline with Dagger..."
    @mkdir -p ./release-artifacts
    dagger call release --source . --version {{ version }} --github-org {{ github_org }} --docker-tag {{ docker-tag }} --push-docker {{ push-docker }} export --path ./release-artifacts/
    @echo "✅ Complete release pipeline finished!"
    @echo "📦 Release artifacts:"
    @ls -la ./release-artifacts/


# =====================================
# Zigbuild Cross-Compilation Commands
# =====================================

# Build all platforms using cargo-zigbuild Docker image
[group('zigbuild')]
zigbuild-release version="v0.1.0":
    #!/usr/bin/env bash
    echo "🚀 Building releases for all platforms using cargo-zigbuild..."
    mkdir -p ./release-artifacts
    
    # Generate docker constants before building
    ./scripts/generate_docker_constant.sh
    
    # Build all platforms in a single container to maintain state
    docker run --rm -v $(pwd):/io -w /io ghcr.io/rust-cross/cargo-zigbuild:latest \
        sh -c '
            echo "📦 Adding Rust targets..." && \
            rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu && \
            echo "🔨 Building Linux x86_64..." && \
            cargo zigbuild --release --target x86_64-unknown-linux-gnu --no-default-features && \
            echo "🔨 Building Linux ARM64..." && \
            cargo zigbuild --release --target aarch64-unknown-linux-gnu --no-default-features && \
            echo "🔨 Building macOS x86_64..." && \
            cargo zigbuild --release --target x86_64-apple-darwin --features macos-keychain && \
            echo "🔨 Building macOS ARM64..." && \
            cargo zigbuild --release --target aarch64-apple-darwin --features macos-keychain && \
            echo "🔨 Building macOS Universal Binary..." && \
            cargo zigbuild --release --target universal2-apple-darwin --features macos-keychain && \
            echo "🔨 Building Windows x86_64..." && \
            cargo zigbuild --release --target x86_64-pc-windows-gnu --no-default-features
        '
    
    # Package all builds
    echo "📦 Packaging release artifacts..."
    
    # Linux x86_64
    tar czf ./release-artifacts/claude-task-{{ version }}-x86_64-unknown-linux-gnu.tar.gz \
        -C target/x86_64-unknown-linux-gnu/release claude-task \
        -C "$(pwd)" README.md
    
    # Linux ARM64
    tar czf ./release-artifacts/claude-task-{{ version }}-aarch64-unknown-linux-gnu.tar.gz \
        -C target/aarch64-unknown-linux-gnu/release claude-task \
        -C "$(pwd)" README.md
    
    # macOS x86_64
    tar czf ./release-artifacts/claude-task-{{ version }}-x86_64-apple-darwin.tar.gz \
        -C target/x86_64-apple-darwin/release claude-task \
        -C "$(pwd)" README.md
    
    # macOS ARM64
    tar czf ./release-artifacts/claude-task-{{ version }}-aarch64-apple-darwin.tar.gz \
        -C target/aarch64-apple-darwin/release claude-task \
        -C "$(pwd)" README.md
    
    # macOS Universal
    tar czf ./release-artifacts/claude-task-{{ version }}-universal2-apple-darwin.tar.gz \
        -C target/universal2-apple-darwin/release claude-task \
        -C "$(pwd)" README.md
    
    # Windows x86_64 (use zip for Windows convention)
    zip -j ./release-artifacts/claude-task-{{ version }}-x86_64-pc-windows-gnu.zip \
        target/x86_64-pc-windows-gnu/release/claude-task.exe \
        README.md
    
    echo "✅ All platform releases built successfully!"
    echo "📦 Release artifacts:"
    ls -la ./release-artifacts/

# Test zigbuild setup for a single platform
[group('zigbuild')]
zigbuild-test target="x86_64-apple-darwin":
    #!/usr/bin/env bash
    echo "🧪 Testing cargo-zigbuild for {{ target }}..."
    
    # Generate docker constants before building
    ./scripts/generate_docker_constant.sh
    
    # Determine feature flags based on target
    if [[ "{{ target }}" == *"apple-darwin"* ]]; then
        features="--features macos-keychain"
    else
        features="--no-default-features"
    fi
    
    docker run --rm -v $(pwd):/io -w /io ghcr.io/rust-cross/cargo-zigbuild:latest \
        sh -c "rustup target add {{ target }} && cargo zigbuild --release --target {{ target }} $features"
    
    # Determine binary name based on target
    if [[ "{{ target }}" == *"windows"* ]]; then
        binary_name="claude-task.exe"
    else
        binary_name="claude-task"
    fi
    
    echo "✅ Build successful! Binary at: target/{{ target }}/release/$binary_name"



# Task Management Commands

# Run a Claude task Example: `just task "Analyze the codebase" --debug`
[group('task')]
task prompt *args: build
    @echo "🤖 Running Claude task: {{prompt}}"
    cargo run -- run "{{prompt}}" {{args}}

# Git Submodule Commands

# Recursively sync git submodules to their specified branches
[group('git')]
sync-modules:
    @echo "🔄 Syncing git submodules..."
    git submodule update --init --recursive --remote

# Docker Commands

# Build Docker image using buildx
[group('docker')]
docker-bake:
    @GITHUB_ORG={{github_org}} docker buildx bake -f docker/docker-bake.hcl

# Test Docker setup
[group('docker')]
test-docker:
    @echo "🐳 Testing Docker setup..."
    cd scripts && ./test-docker.sh

# Login to GitHub Container Registry
[group('docker')]
docker-login username="$USER":
    @echo "🔐 Logging in to GitHub Container Registry..."
    @echo "Please ensure GITHUB_TOKEN environment variable is set"
    @echo $GITHUB_TOKEN | docker login ghcr.io -u {{username}} --password-stdin

# Build and push to GHCR
[group('docker')]
docker-push: docker-login
    @echo "🚀 Building and pushing to GHCR..."
    VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/') \
    GITHUB_ORG={{github_org}} \
    docker buildx bake -f docker/docker-bake.hcl claude-task-ghcr --push

# Build and push all variants to GHCR
[group('docker')]
docker-push-all: docker-login
    @echo "🚀 Building and pushing all variants to GHCR..."
    VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/') \
    GITHUB_ORG={{github_org}} \
    docker buildx bake -f docker/docker-bake.hcl ghcr --push

# Pull latest image from GHCR
[group('docker')]
docker-pull variant="latest":
    @echo "📥 Pulling {{docker_image}}:{{variant}}..."
    docker pull {{docker_image}}:{{variant}}

