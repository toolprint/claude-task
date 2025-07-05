#!/bin/bash
# Debug script for HT-MCP integration

echo "Running HT-MCP debug test..."
echo ""
echo "This will run claude-task with debug logging enabled to help diagnose issues."
echo ""

# Simple prompt that explicitly mentions using ht-mcp
PROMPT="Please use the ht-mcp MCP server to create a new terminal session and run 'ls -la'. Do NOT use the built-in Bash tool."

echo "Running with debug mode enabled..."
./run-with-ht-mcp.sh -d 3618 "$PROMPT"