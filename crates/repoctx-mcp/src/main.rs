//! RepoCtx MCP server — JSON-RPC over stdio (stub for v0 scaffolding).

use std::io::{self, Write};

fn main() {
    // v0: placeholder that documents the intended MCP surface until rmcp is wired.
    let _ = writeln!(
        io::stderr(),
        "repoctx-mcp: MCP server scaffolding — use CLI until rmcp integration lands"
    );
}
