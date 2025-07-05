# HT-MCP Integration Troubleshooting

## New Dynamic Configuration Approach

The integration now uses `claude mcp add-json` to dynamically configure MCP servers instead of static config files. This eliminates the need for complex file mounting.

## Debug Steps

When running with debug mode (`-d` flag), the container will:

1. **Validate HT-MCP binary**
   - Check if binary exists
   - Run version check
   - Show any errors

2. **Configure MCP servers dynamically**
   - Add CCO approval server using `claude mcp add-json`
   - Add HT-MCP server using `claude mcp add-json`
   - Both are added at user scope

3. **Validate MCP configuration**
   - Run `claude mcp list` to verify servers are registered
   - Show any configuration errors

4. **Run Claude Code**
   - Execute with the configured MCP servers
   - Use CCO approval tool for permissions
   - HT-MCP runs as an MCP server, not a standalone process

## Important: Enabling the Web Interface

**HT-MCP does not run a web server by default!** You must explicitly enable it when creating sessions:

```javascript
// When using ht_create_session, ALWAYS set enableWebServer to true:
ht_create_session({
  enableWebServer: true,  // REQUIRED for web interface on port 3618
  command: "bash"         // optional starting command
})
```

Without `enableWebServer: true`, there will be no web interface to monitor terminal activity.

## Testing the Integration

1. **Build the project**:
   ```bash
   cargo build --release
   ```

2. **Rebuild Docker image** (REQUIRED on first run or after Dockerfile changes):
   ```bash
   ./rebuild-and-test.sh
   ```
   
   This forces a rebuild of the Docker image to include the HT-MCP binary.

3. **Run with debug mode to see all steps**:
   ```bash
   ./run-with-ht-mcp.sh -d -a
   ```

4. **Or use the dedicated approval script**:
   ```bash
   ./run-ht-mcp-with-approval.sh -d
   ```

## What to Look for in Debug Output

✅ **Success indicators**:
- "✓ HT-MCP found at: /usr/local/bin/ht-mcp"
- "✓ HT-MCP server is running"
- MCP servers listed when running `claude mcp list`
- Web interface accessible at http://localhost:3618

❌ **Failure indicators**:
- "✗ HT-MCP binary not found" - **Docker image needs rebuild, run `./rebuild-and-test.sh`**
- "⚠️ HT-MCP version check failed"
- "⚠️ Failed to add CCO server"
- "✗ HT-MCP server failed to start"

## Common Issues

### "HT-MCP binary not found"
This means the Docker image doesn't contain the HT-MCP binary. Solution:
```bash
./rebuild-and-test.sh
```
This rebuilds the Docker image with the HT-MCP binary included.

## Alternative: Use musl Binary

If the standard binary doesn't work, try using the musl version:

Edit Dockerfile and change:
```dockerfile
COPY modules/ht-mcp/release/latest/ht-mcp-linux-x86_64 /usr/local/bin/ht-mcp
```
to:
```dockerfile
COPY modules/ht-mcp/release/latest/ht-mcp-linux-x86_64-musl /usr/local/bin/ht-mcp
```

Then rebuild the image.