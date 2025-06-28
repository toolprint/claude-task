use anyhow::Result;

/// Represents a valid approval tool permission format: mcp__server_name__tool_name
#[derive(Debug, Clone)]
pub struct ApprovalToolPermission {
    server_name: String,
    tool_name: String,
}

impl ApprovalToolPermission {
    /// Parse and validate approval tool permission format
    /// Format must be: mcp__<server_name>__<tool_name>
    /// where server_name and tool_name are non-whitespace characters (including single underscores)
    pub fn parse(permission: &str) -> Result<Self> {
        if permission.is_empty() {
            return Err(anyhow::anyhow!("Approval tool permission cannot be empty"));
        }

        let parts: Vec<&str> = permission.split("__").collect();

        if parts.len() != 3 {
            return Err(anyhow::anyhow!(
                "Invalid approval tool permission format. Expected: mcp__<server_name>__<tool_name>, got: {}", 
                permission
            ));
        }

        let [prefix, server_name, tool_name] = [parts[0], parts[1], parts[2]];

        // First part must always be "mcp"
        if prefix != "mcp" {
            return Err(anyhow::anyhow!(
                "Approval tool permission must start with 'mcp', got: {}",
                prefix
            ));
        }

        // Server name and tool name must be non-empty and contain no whitespace
        if server_name.is_empty() {
            return Err(anyhow::anyhow!(
                "Server name cannot be empty in approval tool permission: {}",
                permission
            ));
        }

        if tool_name.is_empty() {
            return Err(anyhow::anyhow!(
                "Tool name cannot be empty in approval tool permission: {}",
                permission
            ));
        }

        if server_name.chars().any(char::is_whitespace) {
            return Err(anyhow::anyhow!(
                "Server name cannot contain whitespace in approval tool permission: {}",
                permission
            ));
        }

        if tool_name.chars().any(char::is_whitespace) {
            return Err(anyhow::anyhow!(
                "Tool name cannot contain whitespace in approval tool permission: {}",
                permission
            ));
        }

        // Check for double underscores within server_name or tool_name
        if server_name.contains("__") {
            return Err(anyhow::anyhow!(
                "Server name cannot contain double underscores '__' in approval tool permission: {}", 
                permission
            ));
        }

        if tool_name.contains("__") {
            return Err(anyhow::anyhow!(
                "Tool name cannot contain double underscores '__' in approval tool permission: {}",
                permission
            ));
        }

        Ok(ApprovalToolPermission {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
        })
    }
}

impl std::fmt::Display for ApprovalToolPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mcp__{}__{}", self.server_name, self.tool_name)
    }
}
