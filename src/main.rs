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
            let config = datap4k_mcp::config::Config::load()?;
            let indexer = datap4k_mcp::index::Indexer::open(&config)?;
            let server = datap4k_mcp::server::DataP4kServer::new(indexer, config);
            server.run().await?;
        }
        Command::Index { path, version } => {
            let version = version.unwrap_or_else(|| {
                let dirname = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                datap4k_mcp::model::version_from_dirname(&dirname)
                    .map(|(v, _)| v)
                    .unwrap_or(dirname)
            });
            tracing::info!("Indexing {path} as version {version}");

            let mut config = datap4k_mcp::config::Config::load()?;
            let indexer = datap4k_mcp::index::Indexer::open(&config)?;
            let stats = indexer.index_directory(&path, &version, "auto")?;

            config.add_version(&path, &version, "auto");
            config.save()?;

            println!(
                "Indexed {}: {} entities, {} edges, {} warnings",
                stats.version, stats.node_count, stats.edge_count, stats.warning_count
            );
        }
    }

    Ok(())
}
