/// Bundled assets for Claude Task
/// This module contains files that are embedded into the binary at compile time

/// The CLAUDE.md content that will be written to ~/.claude/CLAUDE.md during setup
/// This provides User Memory instructions for Claude Code when running in the container
pub const CLAUDE_MD: &str = include_str!("assets/CLAUDE.md");

/// Get the CLAUDE.md content that should be written during setup
pub fn get_claude_md_content() -> &'static str {
    CLAUDE_MD
}
