//! (a) The fake meloet as a plain stdio MCP server.

use rmcp::{ServiceExt, transport::stdio};
use s10_mcp::FakeMeloet;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = FakeMeloet::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
