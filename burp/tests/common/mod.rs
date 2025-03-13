use std::{fs::File, io::Read, path::PathBuf, str::FromStr};

use burp::{
    graph::{oracle::Oracle, PoiGraph},
    types::{CoordNode, Poi},
};
use graph_rs::graph::csr::DirectedCsrGraph;

pub fn setup() -> (
    PoiGraph<Poi>,
    Oracle<DirectedCsrGraph<f64, CoordNode<f64, Poi>>>,
) {
    let oracle_path = PathBuf::from_str("../resources/small.ocl").unwrap();
    let graph_path = PathBuf::from_str("../resources/small_poi.gfb").unwrap();
    let mut f_buf = vec![];
    File::open(oracle_path)
        .unwrap()
        .read_to_end(&mut f_buf)
        .unwrap();

    let mut oracle: Oracle<DirectedCsrGraph<f64, CoordNode<f64, Poi>>> =
        Oracle::read_flexbuffer(f_buf.as_slice());

    let mut f_buf = vec![];
    File::open(graph_path)
        .unwrap()
        .read_to_end(&mut f_buf)
        .unwrap();

    let graph: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());

    (graph, oracle)
}
