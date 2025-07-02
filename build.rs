use std::env;
use std::fs;
use std::path::Path;
use syn::{visit::Visit, Attribute, ItemImpl, Lit, Meta};

/// AST visitor that extracts MCP tool information from the source code
struct ToolExtractor {
    tools: Vec<(String, String)>,
}

impl ToolExtractor {
    fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Extract description from #[tool(description = "...")] attributes
    fn extract_description_from_attrs(attrs: &[Attribute]) -> Option<String> {
        for attr in attrs {
            if attr.path().is_ident("tool") {
                // Parse the attribute arguments
                if let Ok(args) = attr.parse_args::<Meta>() {
                    if let Meta::NameValue(nv) = args {
                        if nv.path.is_ident("description") {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let Lit::Str(lit_str) = &expr_lit.lit {
                                    return Some(lit_str.value());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl<'ast> Visit<'ast> for ToolExtractor {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        // Check if this impl block has the #[tool_router] attribute
        let has_tool_router = node.attrs.iter().any(|attr| attr.path().is_ident("tool_router"));
        
        if has_tool_router {
            // Visit all items in the impl block
            for item in &node.items {
                if let syn::ImplItem::Fn(method) = item {
                    // Check if this method has a #[tool] attribute
                    if let Some(description) = Self::extract_description_from_attrs(&method.attrs) {
                        let fn_name = method.sig.ident.to_string();
                        self.tools.push((fn_name, description));
                    }
                }
            }
        }
        
        // Continue visiting nested items
        syn::visit::visit_item_impl(self, node);
    }
}

fn main() {
    // Ensure the build script reruns if source files change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/mcp.rs");

    // Extract tools from the MCP source file by parsing the AST
    let tools = extract_tools_from_mcp();
    
    // Generate MCP help text at compile time
    let mcp_help = generate_mcp_help_text(tools);
    
    // Write the generated help text as a Rust constant
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("mcp_help.rs");
    
    let content = format!(
        "/// Generated MCP help text\npub const MCP_HELP_TEXT: &str = r###\"{}\"###;\n",
        mcp_help
    );
    
    fs::write(&dest_path, content).unwrap();
}

/// Parse src/mcp.rs and extract all MCP tool names and descriptions
fn extract_tools_from_mcp() -> Vec<(String, String)> {
    // Read the MCP source file
    let mcp_content = fs::read_to_string("src/mcp.rs")
        .expect("Failed to read src/mcp.rs");
    
    // Parse the file into an AST
    let syntax_tree = syn::parse_file(&mcp_content)
        .expect("Failed to parse src/mcp.rs");
    
    // Extract tools using our visitor pattern
    let mut extractor = ToolExtractor::new();
    extractor.visit_file(&syntax_tree);
    
    // Sort tools by name for consistent output
    let mut tools = extractor.tools;
    tools.sort_by(|a, b| a.0.cmp(&b.0));
    
    tools
}

/// Generate the formatted help text with ANSI styling
fn generate_mcp_help_text(tools: Vec<(String, String)>) -> String {
    let mut help_text = String::new();
    help_text.push_str("\n\x1b[1;4mTools:\x1b[0m\n");
    
    // Find the longest tool name for alignment
    let max_name_len = tools.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    
    for (name, description) in tools {
        help_text.push_str(&format!("  \x1b[1m{:<width$}\x1b[0m  {}\n", name, description, width = max_name_len));
    }
    
    help_text.push_str("\nThese tools are exposed via the MCP protocol when running 'ct mcp'.\n");
    
    help_text
}