use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use burp::{
    graph::{
        oracle::{self, Oracle},
        PoiGraph,
    },
    input::geo_zero::GraphWriter,
    types::Poi,
};
use clap::{Args, Parser, Subcommand};
use geozero::geojson::read_geojson;
use graph_rs::{
    algorithms::trajan_scc::TarjanSCC,
    graph::{quad_tree::QuadGraph, rstar::RTreeGraph},
    Graph,
};
use indicatif::ProgressBar;
use rand::prelude::*;

#[derive(Parser)]
struct Cli {
    /// Build .ocl file from geo-json
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum Commands {
    Graph { in_file: PathBuf, out_file: PathBuf },
    Build { graph: PathBuf, epsilon: f64 },
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    match cli.command {
        Commands::Graph { in_file, out_file } => {
            let file = File::open(in_file).unwrap();
            let reader = BufReader::new(file);
            let mut graph_writer = GraphWriter::default();
            read_geojson(reader, &mut graph_writer).unwrap();

            let graph = PoiGraph::new(RTreeGraph::new_from_graph(graph_writer.get_graph()));

            let mut writer = BufWriter::new(File::create(out_file).unwrap());
            writer.write_all(graph.to_flexbuffer().as_slice()).unwrap();
        }
        Commands::Build { graph, epsilon } => {
            let mut file = File::open(graph).unwrap();
            let mut f_buf = vec![];
            file.read_to_end(&mut f_buf).unwrap();
            let graph: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());

            let node = thread_rng().gen_range(0..graph.graph().node_count());

            let oracle = oracle::build(&mut graph.graph_mut(), node, epsilon);
        }
    }
}
