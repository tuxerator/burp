use std::{
    fs::{File, OpenOptions},
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
use memmap2::MmapOptions;
use rand::{prelude::*, rng, seq::index::sample};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

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

        /// Sample <NUMBER> pois form nodes at random.
        #[arg(short, long, conflicts_with = "pois")]
        sample: Option<usize>,
    },
    Build {
        /// Input graph in '.gfb' format
        in_file: PathBuf,

        /// set epsilon
        #[arg(short, long, value_name = "FLOAT")]
        epsilon: f64,

        /// Set output file to <FILE>. Defaults to '<IN_FILE>.ocl'.
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
            sample: sample_size,
        } => {
            let out_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("gmp");
                out_file
            });
            let in_file = File::open(in_file).unwrap();
            let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

            let mut graph_writer = GraphWriter::default();

            read_geojson(in_file_mmap.as_ref(), &mut graph_writer).unwrap();

            let mut graph = PoiGraph::new(RTreeGraph::new_from_graph(graph_writer.get_graph()));

            if let Some(pois) = pois {
                let mut poi_writer = PoiWriter::new(|_| true);
                read_geojson(BufReader::new(File::open(pois).unwrap()), &mut poi_writer).unwrap();
                graph.add_coord_pois(poi_writer.pois()).unwrap();
            }

            if let Some(sample_size) = sample_size {
                let pois = sample(&mut rng(), graph.graph().node_count(), sample_size)
                    .into_iter()
                    .map(|node_id| (node_id, vec![]))
                    .collect();
                graph.add_node_pois(pois)
            }

            let out_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(out_file)
                .unwrap();

            let mut writer = BufWriter::new(out_file);
            let mut rmp_serializer = Serializer::new(writer);
            graph.serialize(&mut rmp_serializer);
        }
        Commands::Build {
            in_file,
            out_file,
            epsilon,
        } => {
            let out_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("omp");
                out_file
            });

            let in_file = File::open(in_file).unwrap();
            let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

            let mut rmp_deserializer = Deserializer::new(in_file_mmap.as_ref());

            let mut graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut rmp_deserializer).unwrap();

            let mut oracle = oracle::Oracle::new();

            oracle.build_for_nodes(
                &mut graph.graph,
                &graph.poi_nodes,
                epsilon,
                Some(ProgressBar::new(0)),
            );

            let mut writer = BufWriter::new(File::create(out_file).unwrap());
            let mut rmp_serializer = Serializer::new(writer);
            oracle.serialize(&mut rmp_serializer);
        }
    }
}
