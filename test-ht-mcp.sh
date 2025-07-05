#!/bin/bash
# Test script for HT-MCP integration

echo "Testing HT-MCP integration in claude-task..."
echo

# Build the project first
echo "1. Building the project..."
cargo build --release || { echo "Build failed"; exit 1; }
echo "✓ Build successful"
echo

# Run setup to ensure credentials are configured
echo "2. Running setup..."
./target/release/claude-task setup || { echo "Setup failed"; exit 1; }
echo "✓ Setup complete"
echo

# Test running with HT-MCP port exposed
echo "3. Testing claude-task with HT-MCP..."
echo "   You can now run the HT-MCP test using:"
echo
echo "   RECOMMENDED - With CCO approval tool:"
echo "   ./run-with-ht-mcp.sh -a                                 # Use defaults with approval"
echo "   ./run-with-ht-mcp.sh -a -d                             # Debug mode with approval"
echo "   ./run-ht-mcp-with-approval.sh                          # Dedicated approval script"
echo
echo "   WITHOUT approval (not recommended - Claude can bypass HT-MCP):"
echo "   ./run-with-ht-mcp.sh                                    # Use defaults (port 3618)"
echo "   ./run-with-ht-mcp.sh 8080                              # Use port 8080"
echo "   ./run-with-ht-mcp.sh 3618 'Create a Python hello world' # Custom prompt"
echo
echo "   Run './run-with-ht-mcp.sh --help' for more information"