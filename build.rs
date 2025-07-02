use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Generate MCP help text at compile time
    let mcp_help = generate_mcp_help_text();
    
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("mcp_help.rs");
    
    let content = format!(
        "/// Generated MCP help text\npub const MCP_HELP_TEXT: &str = r###\"{}\"###;\n",
        mcp_help
    );
    
    fs::write(&dest_path, content).unwrap();
    
    // Tell Cargo to rerun this script if it changes
    println!("cargo:rerun-if-changed=build.rs");
}

fn generate_mcp_help_text() -> String {
    let tools = vec![
        ("setup", "Setup claude-task with your current environment"),
        ("create_worktree", "Create a git worktree for a task"),
        ("list_worktree", "List git worktrees"),
        ("remove_worktree", "Remove a git worktree"),
        ("init_docker_volume", "Initialize shared Docker volumes for Claude tasks"),
        ("list_docker_volume", "List Docker volumes for Claude tasks"),
        ("clean_docker_volume", "Clean up all shared Docker volumes"),
        ("run_task", "Run a Claude task in a local docker container"),
        ("clean", "Clean up all claude-task git worktrees and docker volumes"),
    ];
    
    let mut help_text = String::new();
    help_text.push_str("\n\x1b[1;4mTools:\x1b[0m\n");
    
    // Find the longest tool name for alignment
    let max_name_len = tools.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    
    for (name, description) in tools {
        help_text.push_str(&format!("  \x1b[1m{:<width$}\x1b[0m  {}\n", name, description, width = max_name_len));
    }
    
    help_text.push_str("\nThese tools are exposed via the MCP protocol when running 'ct mcp'.\n");
    help_text.push_str("They can be accessed by MCP clients like Claude Desktop.");
    
    help_text
}