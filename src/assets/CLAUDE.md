# CLAUDE.md

INSTRUCTIONS PRIOR TO READING AND EXECUTING ANY PROMPTS:

First, check if HT-MCP tools are available in this environment:

1. Try to list the available MCP servers by running `claude mcp list` or check if 'ht-mcp' tools are available
2. If HT-MCP tools are available and functional:
   - You are NOT allowed to use your internal tools for terminal access, such as Bash(), LS(), or Grep()
   - Immediately open a single session with 'ht-mcp' tools and note the session-id
   - Use this session-id for the entire session unless it fails (you should only have one open at a time)
   - ALWAYS use the 'use web server' flag set to true when creating a session
   - Follow the HT-MCP guidelines below
3. If HT-MCP tools are NOT available or non-functional:
   - You may use your built-in tools such as Bash(), LS(), or Grep() for terminal interaction
   - Proceed with normal Claude Code operation using built-in tools
   - Inform the user that HT-MCP is not available but tasks can still be completed

## HT-MCP Guidelines (when HT-MCP is available)

It is CRITICAL that you MUST use the 'ht-mcp' tools for ALL interaction with a local terminal when HT-MCP is available and functional. You do NOT have permission to use your built-in tools such as Bash(), LS(), or Grep() to execute commands in this case.

HT-MCP is a "headless terminal" that simulates the experience a human sees, especially for terminal applications that function as a 'screen'. By using this instead you get a better view of certain terminal applications.

An additional critical reason you are to use the 'ht-mcp' tools when available, is that it allows humans to see what commands you execute via a Web View. You must ALWAYS set the "use web server" flag to "true" if that option is provided in an 'ht-mcp' tool. The user cannot see what you are doing unless you provide this web view.

## Fallback Operation (when HT-MCP is not available)

When HT-MCP tools are not available or non-functional, you should:

- Use your built-in tools (Bash, LS, Grep, etc.) normally
- Inform the user that HT-MCP web monitoring is not available
- Complete tasks using standard Claude Code functionality
- Provide clear terminal output and explanations of commands being run
