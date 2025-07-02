use std::{fs::File, io::Read, path::PathBuf, str::FromStr};

use burp::{
    oracle::{oracle::Oracle, PoiGraph},
    types::{CoordNode, Poi},
};
use graph_rs::graph::csr::DirectedCsrGraph;
use memmap2::MmapOptions;
use rmp_serde::Deserializer;
use serde::Deserialize;

pub fn setup() -> (PoiGraph<Poi>, Oracle) {
    let graph_file = File::open("../resources/small_poi.gmp").unwrap();
    let oracle_file = File::open("../resources/small_poi.omp").unwrap();

    let graph_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };
    let oracle_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };

    let mut graph_deser = Deserializer::from_read_ref(&graph_mmap);
    let mut oracle_deser = Deserializer::from_read_ref(&oracle_mmap);

    let graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut graph_deser).unwrap();
    let oracle: Oracle<f64> = Oracle::deserialize(&mut oracle_deser).unwrap();

    (graph, oracle)
}
