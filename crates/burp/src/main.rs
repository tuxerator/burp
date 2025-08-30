use std::{
    fmt::Display,
    fs::{File, OpenOptions},
    io::BufWriter,
    path::PathBuf,
};

use burp::{
    input::{
        self,
        geo_zero::{GraphWriter, PoiWriter},
    },
    oracle::{
        DefaultOracleParams, PoiGraph, SimpleSplitStrategy,
        oracle::{self, Oracle, OracleCollection},
    },
    types::Poi,
};
use clap::{Parser, Subcommand};
use geozero::geojson::read_geojson;
use graph_rs::{Graph, graph::rstar::RTreeGraph};
use indicatif::ProgressBar;
use log::{debug, info};
use memmap2::MmapOptions;
use rand::{prelude::*, rng, seq::index::sample};
use rayon::iter::IntoParallelRefIterator;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

mod bench;

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

        /// Set output file to <FILE>. Defaults to '<IN_FILE>.gmp'.
        #[arg(short = 'o', long)]
        out_file: Option<PathBuf>,

        /// Node coords in '.co' format when providing graph in '.gr' format.
        #[arg(short, long)]
        coords_file: Option<PathBuf>,

        /// Points of intrest.
        #[arg(short, long)]
        pois: Option<PathBuf>,

        /// Sample <NUMBER> pois form nodes at random.
        #[arg(short, long, conflicts_with = "pois")]
        sample: Option<usize>,
    },
    Build {
        /// Input graph in '.gmp' format
        in_file: PathBuf,

        /// set epsilon
        #[arg(short, long, value_name = "FLOAT")]
        epsilon: f64,

        /// Save split-tree to '<OUT_FILE>.smp'
        #[arg(short, long)]
        split_tree: bool,

        /// Compress oracle through merging in-path blocks
        #[arg(short, long)]
        merge_blocks: bool,

        /// Set output file to <FILE>. Defaults to '<IN_FILE>.omp'.
        #[arg(short = 'o', long)]
        out_file: Option<PathBuf>,
    },

    Bench {
        in_file: PathBuf,
        /// Measure oracle size
        #[arg(short, long)]
        size: bool,

        #[arg(short, long)]
        batch_size: u64,
    },
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let working_dir = std::env::current_dir().unwrap();
    log::info!("Current working dir '{}'", working_dir.display());

    match cli.command {
        Commands::Graph {
            in_file,
            out_file,
            coords_file,
            pois,
            sample: sample_size,
        } => {
            info!("Build graph from {:?}", in_file);
            let out_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("gmp");
                out_file
            });
            let file_extension = in_file
                .extension()
                .expect("in-file is missing a file extension")
                .to_owned();
            let in_file = File::open(in_file).unwrap();
            let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

            let mut graph;

            match file_extension
                .to_str()
                .expect("Cannot convert file_extension to 'str'")
            {
                "geojson" => {
                    let mut graph_writer = GraphWriter::default();

                    read_geojson(in_file_mmap.as_ref(), &mut graph_writer).unwrap();
                    graph = PoiGraph::new(RTreeGraph::new_from_graph(graph_writer.get_graph()));
                }
                ext => panic!("file type '.{ext}' not supported"),
            }

            if let Some(pois) = pois {
                panic!("Pois are not read correctly at the moment!");
                // let mut poi_writer = PoiWriter::new(|_| true);
                // read_geojson(BufReader::new(File::open(pois).unwrap()), &mut poi_writer).unwrap();
                // graph.add_coord_pois(poi_writer.pois()).unwrap();
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

            // let mut cache = DijkstraCache::new(graph.graph());
            //
            // drop(graph);
            //
            // cache.dijkstra_cached(0, FxHashSet::from_iter([1]), Direction::Outgoing);
        }
        Commands::Build {
            in_file,
            out_file,
            epsilon,
            split_tree,
            merge_blocks,
        } => {
            let oracle_file = out_file.unwrap_or_else(|| {
                let mut out_file = in_file.clone();
                out_file.set_extension("omp");
                out_file
            });

            let in_file = File::open(in_file).unwrap();
            let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

            let mut rmp_deserializer = Deserializer::new(in_file_mmap.as_ref());

            let mut graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut rmp_deserializer).unwrap();
            info!(
                "Loaded graph: {} nodes, {} edges",
                graph.graph().node_count(),
                graph.graph().edge_count()
            );

            let mut oracles = OracleCollection::default();

            let split_trees = oracles
                .build_for_nodes(
                    graph.poi_nodes(),
                    epsilon,
                    graph.graph(),
                    DefaultOracleParams { merge_blocks },
                )
                .unwrap();

            if split_tree {
                for split_tree in split_trees.iter() {
                    let mut file_name = oracle_file.file_stem().unwrap().to_os_string();
                    file_name.push(format!("_{}", split_tree.0));

                    let mut split_tree_file = oracle_file.parent().unwrap().to_path_buf();
                    split_tree_file.push("split_trees");
                    split_tree_file.push(file_name.as_os_str());
                    split_tree_file.set_extension("smp");

                    std::fs::create_dir_all(split_tree_file.parent().unwrap());

                    let writer = BufWriter::new(File::create(split_tree_file).unwrap());
                    let mut rmp_serializer = Serializer::new(writer);
                    split_tree.serialize(&mut rmp_serializer).unwrap();
                }
            }

            for oracle in oracles.iter() {
                let mut file_name = oracle_file.file_stem().unwrap().to_os_string();
                file_name.push(format!("_{}", oracle.0));

                let mut oracle_file = oracle_file.clone();
                oracle_file.set_file_name(file_name.as_os_str());
                oracle_file.set_extension("omp");

                let writer = BufWriter::new(File::create(oracle_file).unwrap());
                let mut rmp_serializer = Serializer::new(writer);
                oracle.1.serialize(&mut rmp_serializer).unwrap();
            }
        }
        Commands::Bench {
            in_file,
            size,
            batch_size,
        } => {
            if size {
                struct Measurements(Vec<(f64, f64)>);
                impl Display for Measurements {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        self.0.iter().for_each(|e| {
                            writeln!(f, "{} \t {}", e.0, e.1).unwrap_or_else(|e| debug!("{e}"));
                        });
                        Ok(())
                    }
                }
                println!(
                    "Merged: {}",
                    Measurements(bench::oracle_size_merge(
                        &in_file,
                        &[
                            0.05, 0.1, 0.2, 0.25, 0.3, 0.4, 0.5, 0.75, 1., 2., 3., 4., 5.
                        ],
                        batch_size
                    ))
                );

                println!(
                    "Unmerged: {}",
                    Measurements(bench::oracle_size_no_merge(
                        &in_file,
                        &[
                            0.05, 0.1, 0.2, 0.25, 0.3, 0.4, 0.5, 0.75, 1., 2., 3., 4., 5.
                        ],
                        batch_size
                    ))
                );
            }
        }
    }
}
