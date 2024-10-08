use std::{fs::File, io::Read, path::PathBuf};

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

    let nodes = sample(&mut thread_rng(), graph.graph().node_count(), 100);

    for node in nodes {
        group.sample_size(10);
        group.bench_with_input(BenchmarkId::new("poi_oracle", node), &node, |b, n| {
            b.iter(|| oracle::build(&graph.graph(), *n, 0.1))
        });
    }
}

criterion_group!(oracle, build_poi_oracle);
criterion_main!(oracle);
