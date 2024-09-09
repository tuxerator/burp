use std::{
    env,
    fs::{self, File},
    io::{BufReader, Read},
    path::PathBuf,
};

use burp::{oracle::Oracle, types::Poi};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use graph_rs::Graph;
use rand::{seq::index::sample, thread_rng};

pub fn dijkstra_full_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("dijkstra");

    let path = PathBuf::from("../resources/berlin.ocl");
    println!("{}", env::current_dir().unwrap().display());
    let mut file = File::open(path).unwrap();
    let mut f_buf = vec![];
    file.read_to_end(&mut f_buf);
    let oracle: Oracle<Poi> = Oracle::read_flexbuffer(f_buf.as_slice());

    let nodes = sample(&mut thread_rng(), oracle.graph().node_count(), 100);

    for node in nodes {
        group.sample_size(10);
        group.bench_with_input(BenchmarkId::new("dijkstra_full", &node), &node, |b, n| {
            b.iter(|| oracle.dijkstra_full(node))
        });
    }

    group.finish();
}

criterion_group!(dijkstra, dijkstra_full_bench);
criterion_main!(dijkstra);
