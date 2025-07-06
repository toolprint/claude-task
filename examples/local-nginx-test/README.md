# Local Nginx + HT-MCP Test Setup

This directory contains tools to test the nginx proxy configuration locally before using it in Docker.

## Purpose

Test that nginx can properly proxy HT-MCP's web interface, including WebSocket connections and CORS handling.

## Setup

1. **Install dependencies:**
   ```bash
   # macOS
   brew install nginx
   
   # Ubuntu
   sudo apt install nginx
   ```

2. **Build HT-MCP:**
   ```bash
   cd modules/ht-mcp
   cargo build --release
   cd ../../examples/local-nginx-test
   ```

## Testing Process

### Terminal 1: Start Nginx Proxy
```bash
chmod +x start-nginx.sh
./start-nginx.sh
```

This starts nginx listening on port 3618, proxying to localhost:3619.

### Terminal 2: Start Claude Code or MCP Client
Start Claude Code or another long-running MCP client to use the HT-MCP binary to launch a session with a web server. We don't have a script for this in this repo at this time.

Configure your MCP client to use the HT-MCP binary and create a session using the `ht_create_session` tool with:
```json
{
  "enableWebServer": true
}
```

### Testing Steps

1. **Start your MCP client** (Claude Code, etc.) with HT-MCP configured
2. **Create a session** using the `ht_create_session` tool with `enableWebServer: true`
3. **Test the proxy** - Open http://localhost:3618 in your browser
4. **Verify functionality** - You should see the terminal interface through nginx

## What This Tests

- ✅ Nginx proxy configuration
- ✅ WebSocket proxying for terminal connections  
- ✅ CORS headers for cross-origin access
- ✅ Port forwarding (3618 -> 3619)
- ✅ HT-MCP web server binding to 0.0.0.0

## Expected Results

- **nginx access logs** should show incoming requests
- **HT-MCP web interface** should load at http://localhost:3618
- **Terminal interactions** should work through the proxy
- **No CORS errors** in browser console

## Troubleshooting

- **Port conflicts:** Check if anything is using port 3618 or 3619
- **Nginx errors:** Check `nginx-error.log` in this directory
- **HT-MCP issues:** Verify the binary exists and has bind-address support

## Files

- `nginx-local.conf` - Nginx configuration for local testing
- `start-nginx.sh` - Starts nginx proxy server
- `README.md` - This file