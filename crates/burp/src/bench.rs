use std::{fs::File, path::PathBuf, rc::Rc};

use burp::{
    oracle::{PoiGraph, oracle::Oracle},
    types::Poi,
};
use graph_rs::Graph;
use log::info;
use memmap2::MmapOptions;
use rand::{rng, seq::index::sample};
use rmp_serde::Deserializer;
use rustc_hash::FxHashSet;
use serde::Deserialize;

pub fn oracle_size(in_file: PathBuf, v_epsilon: &[f64]) -> Vec<(f64, usize)> {
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
            let mut oracle = Oracle::new();
            let batch = (0..10).map(|_| {
                let poi = sample(&mut rng(), graph.graph().node_count(), 1).index(0);
                oracle.build_for_node(poi, *epsilon, graph.graph());

                oracle.size()
            });

            let mean = batch
                .enumerate()
                .reduce(|a, n| (n.0, a.1 + ((n.1 - a.1) / n.0)));

            (*epsilon, mean.unwrap().1)
        })
        .collect()
}
