use std::{fs::File, io::Read, ops::Deref, path::PathBuf, sync::Arc};

use burp::{
    graph::{
        oracle::{self, Oracle},
        PoiGraph,
    },
    types::Poi,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use graph_rs::{types::Direction, Graph};
use rand::{seq::index::sample, thread_rng};

pub fn build_poi_oracle(c: &mut Criterion) {
    let mut group = c.benchmark_group("oracle");

    let path = PathBuf::from("../resources/berlin.ocl");
    let mut file = File::open(path).unwrap();
    let mut f_buf = vec![];
    file.read_to_end(&mut f_buf);
    let graph: PoiGraph<Poi> = PoiGraph::read_flexbuffer(f_buf.as_slice());
    let mut oracle = Oracle::new(graph.graph_ref());

    let nodes = sample(&mut thread_rng(), graph.graph().node_count(), 100);

    for node in nodes {
        group.sample_size(10);
        group.bench_with_input(BenchmarkId::new("poi_oracle", node), &node, |b, n| {
            b.iter(|| {
                oracle.build_for_node(node, 0.2, None);
            });
        });
    }
}

criterion_group!(oracle, build_poi_oracle);
criterion_main!(oracle);
