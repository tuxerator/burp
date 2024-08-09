use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Input graph as geo-json
    #[arg(short, long, value_name = "FILE", group = "pre")]
    graph: Option<PathBuf>,

    /// Pois as geo-json
    #[arg(short, long, value_name = "FILE", group = "pre")]
    poi: Option<PathBuf>,

    /// Load oracle from file
    #[arg(short, long, value_name = "FILE", conflicts_with = "pre")]
    oracle: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    Dijkstra,
}

fn main() {}
