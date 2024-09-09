use std::{
    fs::File,
    path::{Path, PathBuf},
};

use burp::oracle::Oracle;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    /// Input graph as geo-json
    #[arg(short, long, value_name = "FILE", group = "pre")]
    graph: Option<PathBuf>,

    /// Pois as geo-json
    #[arg(short, long, value_name = "FILE", group = "pre")]
    poi: Option<PathBuf>,

    /// Load oracle from file
    #[arg(short, long, value_name = "FILE", conflicts_with = "pre")]
    oracle: Option<PathBuf>,
    // #[command(subcommand)]
    // command: Commands,
}

// #[derive(Subcommand)]
// enum Commands {
//     /// Find shortest path using the Dijkstra-Algorithm.
//     Dijkstra {
//         start: (f64, f64),
//
//         target: Option<Vec<(f64, f64)>>,
//     },
// }

fn main() {
    todo!();
}
