mod indexer;
mod normalize;
mod parser;
mod search;
mod server;

use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

#[derive(ClapParser)]
#[command(
    author,
    version,
    about = "Search Engine Project - Information Retrieval"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Index {
        #[arg(long)]
        input: PathBuf,

        #[arg(long)]
        out: PathBuf,

        #[arg(long)]
        limit: Option<usize>,
    },
    Serve {
        #[arg(long)]
        index: PathBuf,

        #[arg(long, default_value = "127.0.0.1:8080")]
        host: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { input, out, limit } => {
            println!("🚀 Starting Indexing Process...");
            println!("📂 Input Path: {:?}", input.display());

            let start_time = Instant::now();

            indexer::build_index(&input, &out, limit)?;

            let duration = start_time.elapsed();

            let file_size_mb = if let Ok(metadata) = std::fs::metadata(&out) {
                metadata.len() as f64 / (1024.0 * 1024.0)
            } else {
                0.0
            };

            let separator = "=".repeat(40);
            println!("\n{}", separator);
            println!("✅ Indexing completed successfully.");
            println!("⏱ Time Elapsed: {:.2?}", duration);
            println!("📦 Index File Size: {:.2} MB", file_size_mb);
            println!("💾 Saved to: {:?}", out.display());
            println!("{}", separator);
        }
        Commands::Serve { index, host } => {
            println!("🔄 Loading index from: {:?}", index.display());

            let start_load = Instant::now();
            let idx = indexer::IndexStore::load(&index)?;
            let load_duration = start_load.elapsed();

            println!("✅ Index loaded in {:.2?}", load_duration);
            println!("🌐 Server is running at: http://{}", host);

            server::run_server(idx, host).await?;
        }
    }

    Ok(())
}
