use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "datap4k-mcp", about = "MCP server for Star Citizen p4k game data")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Start the MCP server (stdio transport)
    Serve,
    /// Index a p4k data directory without starting the server
    Index {
        /// Path to extracted p4k data directory
        path: String,
        /// Game version (auto-detected from directory name if omitted)
        #[arg(short, long)]
        version: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            tracing::info!("Starting datap4k-mcp server");
            println!("Server not yet implemented");
        }
        Command::Index { path, version } => {
            tracing::info!("Indexing: {path}");
            println!("Indexer not yet implemented");
        }
    }

    Ok(())
}
