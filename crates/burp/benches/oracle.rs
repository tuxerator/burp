use divan::{AllocProfiler, Bencher};
use geozero::geojson::read_geojson;
use graph_rs::{Graph, graph::rstar::RTreeGraph};
use rand::{SeedableRng, rng, rngs::SmallRng, seq::index::sample};
use std::fs::File;

use burp::oracle::{
    DefaultOracleParams, MinSplitParams, NoMergeParams, OracleParams, oracle::OracleCollection,
};
use rstar::RTreeNum;

use burp::{input::geo_zero::GraphWriter, oracle::PoiGraph, types::Poi};
use memmap2::MmapOptions;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench(
    sample_size = 10,
    sample_count = 10,
    types = [DefaultOracleParams, MinSplitParams, NoMergeParams]
)]
fn oracle<P: OracleParams>(bencher: Bencher) {
    let in_file =
        File::open("../../resources/Konstanz_Paradies_small.geojson").unwrap_or_else(|_| {
            panic!(
                "No such file or directory in '{}'",
                std::env::current_dir().unwrap().display()
            )
        });
    let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

    let mut graph_writer = GraphWriter::default();

    read_geojson(in_file_mmap.as_ref(), &mut graph_writer).unwrap();

    let mut graph = RTreeGraph::new_from_graph(graph_writer.get_graph());

    let params = P::default();

    bencher
        .with_inputs(|| {
            sample(
                &mut SmallRng::seed_from_u64(1),
                graph.graph().node_count(),
                1,
            )
            .into_iter()
            .collect()
        })
        .bench_local_values(|pois| {
            divan::black_box(OracleCollection::default())
                .build_for_nodes(&pois, 0.2, &graph, params)
        });
}
