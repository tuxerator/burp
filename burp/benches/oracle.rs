use geozero::geojson::read_geojson;
use graph_rs::{graph::rstar::RTreeGraph, Graph};
use rand::{rng, seq::index::sample};
use std::{fs::File, iter::successors};

use burp::{
    graph::{
        oracle::{self, Oracle},
        PoiGraph,
    },
    input::geo_zero::GraphWriter,
    types::Poi,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use memmap2::MmapOptions;
use rmp_serde::Deserializer;
use serde::Deserialize;

pub fn build_oracle(c: &mut Criterion) {
    let mut group = c.benchmark_group("oracle");

    let in_file = File::open("../resources/Konstanz_Paradies.geojson").unwrap();
    let in_file_mmap = unsafe { MmapOptions::new().map(&in_file).unwrap() };

    let mut graph_writer = GraphWriter::default();

    read_geojson(in_file_mmap.as_ref(), &mut graph_writer).unwrap();

    let mut graph = PoiGraph::new(RTreeGraph::new_from_graph(graph_writer.get_graph()));

    let size = successors(Some(1), |n| Some(n + 2));

    for size in size.take(10) {
        let pois = sample(&mut rng(), graph.graph().node_count(), size)
            .into_iter()
            .map(|node_id| (node_id, vec![]))
            .collect();
        graph.add_node_pois(pois);

        group.sample_size(10);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_function(BenchmarkId::new("build", size), |b| {
            let mut oracle = Oracle::new();
            b.iter(|| {
                oracle.build_for_nodes(&mut graph.graph, &graph.poi_nodes, 0.2, None);
            });
        });
    }
}

criterion_group!(oracle, build_oracle);
criterion_main!(oracle);
