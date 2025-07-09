#!/usr/bin/env -S just --justfile

_default:
    @just -l -u

# Rust Development Commands

# Build the project
[group('rust')]
build:
    @echo "🔨 Building claude-task..."
    cargo build

# Build in release mode
[group('rust')]
build-release:
    @echo "🔨 Building claude-task (release)..."
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
    
    # 4. Tests (only run if all checks passed)
    echo "4️⃣  Running tests..."
    if cargo test; then
        echo "   ✅ All tests passed"
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

# Build ht-mcp submodule release
[group('git')]
build-ht-mcp version="latest":
    @echo "🔨 Building ht-mcp submodule (version: {{version}})..."
    @if [ ! -d "modules/ht-mcp" ]; then echo "❌ ht-mcp submodule not found. Run 'just sync-modules' first."; exit 1; fi
    cd modules/ht-mcp && chmod +x build-release.sh && ./build-release.sh {{version}}

# Docker Commands

# Prepare ht-mcp binaries for Docker build
[group('docker')]
prepare-docker:
    #!/usr/bin/env bash
    echo "🔨 Preparing ht-mcp binaries for Docker build..."
    
    # Try to build ht-mcp, but continue if it fails (Windows builds might fail on macOS/Linux)
    if just build-ht-mcp; then
        echo "✅ ht-mcp build completed successfully"
    else
        echo "⚠️  ht-mcp build had errors (likely Windows targets), continuing with existing binaries..."
    fi
    
    # Check if we have the required Linux binaries
    if [ ! -f "modules/ht-mcp/release/latest/ht-mcp-linux-x86_64" ] || [ ! -f "modules/ht-mcp/release/latest/ht-mcp-linux-aarch64" ]; then
        echo "❌ Required Linux binaries not found in modules/ht-mcp/release/latest/"
        echo "   Please ensure ht-mcp-linux-x86_64 and ht-mcp-linux-aarch64 exist"
        exit 1
    fi
    
    echo "📁 Copying ht-mcp release files to docker/ht-mcp-release..."
    rm -rf docker/ht-mcp-release
    cp -r modules/ht-mcp/release docker/ht-mcp-release
    echo "✅ Docker build preparation complete"

# Build Docker image using buildx
[group('docker')]
docker-bake:
    @docker buildx bake -f docker/docker-bake.hcl --no-cache

# Test Docker setup
[group('docker')]
test-docker:
    @echo "🐳 Testing Docker setup..."
    cd scripts && ./test-docker.sh

# HT-MCP Commands

# Setup and test HT-MCP integration
[group('ht-mcp')]
test-ht-mcp:
    @echo "🧪 Testing HT-MCP integration..."
    cd scripts && ./test-ht-mcp.sh

# Run claude-task with HT-MCP (with approval tool)
[group('ht-mcp')]
run-ht-mcp prompt port *args:
    #!/usr/bin/env bash
    echo "🚀 Running claude-task with HT-MCP..."
    cd scripts
    PORT={{port}}
    if [ -z "$PORT" ]; then
        PORT=3618
    fi
    if [ -z "{{prompt}}" ]; then
        ./run-with-ht-mcp.sh -a {{args}} $PORT
    else
        ./run-with-ht-mcp.sh -a {{args}} $PORT "{{prompt}}"
    fi

# Run claude-task with HT-MCP in debug mode
[group('ht-mcp')]
run-ht-mcp-debug prompt port *args:
    #!/usr/bin/env bash
    echo "🔍 Running claude-task with HT-MCP (debug mode)..."
    cd scripts
    PORT={{port}}
    if [ -z "$PORT" ]; then
        PORT=3618
    fi
    if [ -z "{{prompt}}" ]; then
        ./run-with-ht-mcp.sh -a -d {{args}} $PORT
    else
        ./run-with-ht-mcp.sh -a -d {{args}} $PORT "{{prompt}}"
    fi

# Start local nginx test environment
[group('ht-mcp')]
test-nginx-local:
    #!/usr/bin/env bash
    echo "🌐 Starting local nginx test environment..."
    echo "Starting nginx proxy in background..."
    cd examples/local-nginx-test
    chmod +x start-nginx.sh
    ./start-nginx.sh &
    NGINX_PID=$!
    cd - > /dev/null
    echo "✅ Nginx proxy started (PID: $NGINX_PID)"
    echo ""
    echo "📋 Next steps:"
    echo "  1. Nginx is now running, proxying localhost:3619 -> localhost:3618"
    echo "  2. In another terminal, start Claude Code with HT-MCP configured"
    echo "  3. Use ht_create_session with enableWebServer: true"
    echo "  4. Open http://localhost:3618 to test the web interface"
    echo ""
    echo "⏹️  Press Ctrl+C to stop nginx and exit"
    echo ""
    
    # Function to cleanup
    cleanup() {
        echo "Stopping nginx (PID: $NGINX_PID)..."
        kill $NGINX_PID 2>/dev/null || true
        exit
    }
    trap cleanup SIGTERM SIGINT
    
    # Wait for user to stop
    wait $NGINX_PID

