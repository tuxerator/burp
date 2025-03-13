use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use burp::{
    graph::{
        oracle::{self, Oracle},
        PoiGraph,
    },
    input::{
        self,
        geo_zero::{GraphWriter, PoiWriter},
    },
    types::Poi,
};
use clap::{Args, Command, Parser, Subcommand};
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
    Graph {
        in_file: PathBuf,

        /// Set output file to <FILE>. Defaults to '<in_file>.gfb'.
        #[arg(short = 'o', long)]
        out_file: Option<PathBuf>,

        /// Points of intrest.
        #[arg(short, long)]
        pois: Option<PathBuf>,
    },
    Build {
        /// Input graph in '.gfb' format
        in_file: PathBuf,

        /// set epsilon
        #[arg(short, long, value_name = "FLOAT")]
        epsilon: f64,

        /// Set output file to <FILE>. Defaults to '<in_file>.ocl'.
        #[arg(short = 'o', long)]
        out_file: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    match cli.command {
        Commands::Graph {
            in_file,
            out_file,
            pois,
        } => {
            let out_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("gfb");
                out_file
            });
            let in_file = File::open(in_file).unwrap();
            let reader = BufReader::new(in_file);
            let mut graph_writer = GraphWriter::default();

            read_geojson(reader, &mut graph_writer).unwrap();

            let mut graph = PoiGraph::new(Arc::new(RwLock::new(RTreeGraph::new_from_graph(
                graph_writer.get_graph(),
            ))));

            if let Some(pois) = pois {
                let mut poi_writer = PoiWriter::new(|_| true);
                read_geojson(BufReader::new(File::open(pois).unwrap()), &mut poi_writer).unwrap();
                graph.add_pois(poi_writer.pois()).unwrap();
            }

            let mut writer = BufWriter::new(File::create(out_file).unwrap());
            writer.write_all(graph.to_flexbuffer().as_slice()).unwrap();
        }
        Commands::Build {
            in_file,
            out_file,
            epsilon,
        } => {
            let out_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("ocl");
                out_file
            });

            let mut f_buf = vec![];
            File::open(in_file)
                .unwrap()
                .read_to_end(&mut f_buf)
                .unwrap();

            let graph: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());

            let mut oracle = oracle::Oracle::new(graph.graph_ref());

            oracle.build_for_points_par(graph.poi_nodes(), epsilon);

            let mut writer = BufWriter::new(File::create(out_file).unwrap());
            writer.write_all(oracle.to_flexbuffer().as_slice()).unwrap();
        }
    }
}
