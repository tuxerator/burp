use std::{fs::File, path::PathBuf, rc::Rc};

use burp::{
    oracle::{DefaultOracleParams, NoMergeParams, PoiGraph, SimpleSplitStrategy, oracle::Oracle},
    types::Poi,
};
use graph_rs::Graph;
use log::info;
use memmap2::MmapOptions;
use rand::{SeedableRng, rng, rngs::SmallRng, seq::index::sample};
use rmp_serde::Deserializer;
use rustc_hash::FxHashSet;
use serde::Deserialize;

pub fn oracle_size_merge(in_file: &PathBuf, v_epsilon: &[f64], batch_size: u64) -> Vec<(f64, f64)> {
    let in_file = File::open(in_file).unwrap();
    let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

    let mut rmp_deserializer = Deserializer::new(in_file_mmap.as_ref());

    let mut graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut rmp_deserializer).unwrap();
    info!(
        "Loaded graph: {} nodes, {} edges",
        graph.graph().node_count(),
        graph.graph().edge_count()
    );

    v_epsilon
        .iter()
        .map(|epsilon| {
            let batch = (0..batch_size).map(|i| {
                let poi = sample(
                    &mut SmallRng::seed_from_u64(i),
                    graph.graph().node_count(),
                    1,
                )
                .index(0);

                let mut oracle =
                    Oracle::build_for_node(poi, *epsilon, graph.graph(), DefaultOracleParams)
                        .unwrap();

                oracle.0.size() as f64
            });

            let mean = batch
                .enumerate()
                .reduce(|a, n| (n.0, a.1 + ((n.1 - a.1) / n.0 as f64)));

            (*epsilon, mean.unwrap().1)
        })
        .collect()
}

pub fn oracle_size_no_merge(
    in_file: &PathBuf,
    v_epsilon: &[f64],
    batch_size: u64,
) -> Vec<(f64, f64)> {
    let in_file = File::open(in_file).unwrap();
    let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

    let mut rmp_deserializer = Deserializer::new(in_file_mmap.as_ref());

    let mut graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut rmp_deserializer).unwrap();
    info!(
        "Loaded graph: {} nodes, {} edges",
        graph.graph().node_count(),
        graph.graph().edge_count()
    );

    v_epsilon
        .iter()
        .map(|epsilon| {
            let batch = (0..batch_size).map(|i| {
                let poi = sample(
                    &mut SmallRng::seed_from_u64(i),
                    graph.graph().node_count(),
                    1,
                )
                .index(0);

                let mut oracle =
                    Oracle::build_for_node(poi, *epsilon, graph.graph(), NoMergeParams).unwrap();

                oracle.0.size() as f64
            });

            let mean = batch
                .enumerate()
                .reduce(|a, n| (n.0, a.1 + ((n.1 - a.1) / n.0 as f64)));

            (*epsilon, mean.unwrap().1)
        })
        .collect()
}
