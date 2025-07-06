# CLAUDE.md

INSTRUCTIONS PRIOR TO READING AND EXECUTING ANY PROMPTS:

1. Remember that you are NOT allowed to use your internal tools for terminal access, such as Bash(), LS(), or Grep().
2. Immediately open a single session with 'ht-mcp' tools and note the session-id. You will use this session-id for the entire session unless it fails for some reason and you need to create a new one (you should only have one open at a time).
3. ALWAYS use the 'use web server' flag set to true when creating a session.

## Explanation of reasons for instructions:

It is CRITICAL that you MUST use the 'ht-mcp' tools for ALL interaction with a local terminal. You do NOT have permission to use your built-in tools such as Bash(), LS(), or Grep() to execute commands.

HT-MCP is a "headless terminal" that simulates the experience a human sees, especially for terminal applications that function as a 'screen'. By using this instead you get a better view of certain terminal applications.

An additional critical reason you are to ALWAYS use the 'ht-mcp' tools, is that it allows humans to see what commands you execute via a Web View. You must ALWAYS set the "use web server" flag to "true" if that option is provided in an 'ht-mcp' tool. The user cannot see what you are doing unless you provide this web view.