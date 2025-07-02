use std::{
    fs::{self, File},
    io::{BufWriter, Read},
    iter::{from_fn, repeat_with, successors},
    path::PathBuf,
    str::FromStr,
};

use burp::{
    oracle::{oracle::Oracle, PoiGraph},
    types::Poi,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geo::{Coord, CoordNum};
use graph_rs::{graph::Path, CoordGraph, Graph};
use memmap2::MmapOptions;
use rand::{
    distr::uniform::{SampleUniform, UniformSampler},
    rng,
    seq::IteratorRandom,
    Rng,
};
use rmp_serde::{Deserializer, Serializer};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

criterion_group!(query, beer_path_small, beer_path_big);
criterion_main!(query);

pub fn beer_path_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("beer-path big");

    let (graph, runs) = setup("../resources/small_poi.gmp".into());

    let envelope = graph.graph().bounding_rect().unwrap();

    let mut rng = rand::rng();
    let dist = UniformCoord::new(SampleCoord(envelope.min()), SampleCoord(envelope.max())).unwrap();

    let mut s_t_iter = repeat_with(|| {
        (
            dist.sample(&mut rng.clone()).coord(),
            dist.sample(&mut rng.clone()).coord(),
        )
    });

    let size = successors(Some(1), |n| Some(n + 1)).map(|n| 2usize.pow(n));

    for run in runs {
        let oracle_file = File::open(run.1).unwrap();

        let oracle_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };

        let mut oracle_deser = Deserializer::from_read_ref(&oracle_mmap);

        let oracle: Oracle<f64> = Oracle::deserialize(&mut oracle_deser).unwrap();

        for sample_rate in run.0 {
            let s_t_pairs: Vec<(Coord, Coord)> = s_t_iter.take(100).collect();
            group.sample_size(100);
            group.throughput(Throughput::Elements(s_t_pairs.len() as u64));
            group.bench_with_input(
                BenchmarkId::new("oracle", sample_rate),
                &s_t_pairs,
                |b, s_t_pairs| {
                    b.iter(|| {
                        s_t_pairs.iter().for_each(|s_t| {
                            oracle.get_pois(&s_t.0, &s_t.1);
                        });
                    })
                },
            );
            group.bench_with_input(
                BenchmarkId::new("dijkstra", sample_rate),
                &s_t_pairs.clone(),
                |b, s_t_pairs| {
                    b.iter(|| {
                        s_t_pairs.iter().for_each(|s_t| {
                            let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                            let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                            graph.beer_path_dijkstra_base(s_node, t_node, graph.poi_nodes(), 0.25);
                        });
                    });
                },
            );
        }
    }
    group.finish();
}

pub fn beer_path_big(c: &mut Criterion) {
    let mut group = c.benchmark_group("beer-path big");

    let (graph, runs) = setup("../resources/medium_poi.gmp".into());

    let envelope = graph.graph().bounding_rect().unwrap();

    let mut rng = rand::rng();
    let dist = UniformCoord::new(SampleCoord(envelope.min()), SampleCoord(envelope.max())).unwrap();

    let mut s_t_iter = repeat_with(|| {
        (
            dist.sample(&mut rng.clone()).coord(),
            dist.sample(&mut rng.clone()).coord(),
        )
    });

    let size = successors(Some(1), |n| Some(n + 1)).map(|n| 2usize.pow(n));

    for run in runs {
        let oracle_file = File::open(run.1).unwrap();

        let oracle_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };

        let mut oracle_deser = Deserializer::from_read_ref(&oracle_mmap);

        let oracle: Oracle<f64> = Oracle::deserialize(&mut oracle_deser).unwrap();

        for sample_rate in run.0 {
            let s_t_pairs: Vec<(Coord, Coord)> = s_t_iter.take(100).collect();
            group.sample_size(100);
            group.throughput(Throughput::Elements(s_t_pairs.len() as u64));
            group.bench_with_input(
                BenchmarkId::new("oracle", sample_rate),
                &s_t_pairs,
                |b, s_t_pairs| {
                    b.iter(|| {
                        s_t_pairs.iter().for_each(|s_t| {
                            oracle.get_pois(&s_t.0, &s_t.1);
                        });
                    })
                },
            );
            group.bench_with_input(
                BenchmarkId::new("dijkstra", sample_rate),
                &s_t_pairs.clone(),
                |b, s_t_pairs| {
                    b.iter(|| {
                        s_t_pairs.iter().for_each(|s_t| {
                            let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                            let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                            graph.beer_path_dijkstra_base(s_node, t_node, graph.poi_nodes(), 0.25);
                        });
                    });
                },
            );
        }
    }
    group.finish();
}

fn setup(graph_path: PathBuf) -> (PoiGraph<Poi>, Vec<(Vec<usize>, PathBuf)>) {
    let graph_file = File::open(graph_path).unwrap();

    let graph_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };

    let mut graph_deser = Deserializer::from_read_ref(&graph_mmap);

    let mut graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut graph_deser).unwrap();

    let mut rng = rng();

    let sampling_rates = [0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5];

    let runs = sampling_rates.map(|sampling_rate| {
        graph
            .graph()
            .nodes_iter()
            .map(|node| node.0)
            .choose_multiple(
                &mut rng,
                (graph.graph().node_count() as f64 * sampling_rate) as usize,
            )
    });

    let runs = runs
        .into_iter()
        .enumerate()
        .map(|run| {
            let mut oracle = Oracle::new();
            oracle.build_for_nodes(
                graph.graph_mut(),
                &FxHashSet::from_iter(run.1.clone()),
                0.25,
                None,
            );

            let tmp_file =
                PathBuf::from_str(&format!("/tmp/burp/oracle_bench_{}.omp", run.0)).unwrap();

            if let Some(parent) = tmp_file.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            let writer = BufWriter::new(File::create(&tmp_file).unwrap());
            let mut rmp_serializer = Serializer::new(writer);
            oracle.serialize(&mut rmp_serializer).unwrap();
            (run.1, tmp_file)
        })
        .collect();
    (graph, runs)
}

struct SampleCoord<C: CoordNum>(Coord<C>);

impl<C: CoordNum> SampleCoord<C> {
    fn coord(self) -> Coord<C> {
        self.0
    }
}

impl<C: CoordNum + SampleUniform> SampleUniform for SampleCoord<C> {
    type Sampler = UniformCoord<C>;
}

struct UniformCoord<C: SampleUniform> {
    x: C::Sampler,
    y: C::Sampler,
}

impl<C: SampleUniform + CoordNum> UniformSampler for UniformCoord<C> {
    type X = SampleCoord<C>;
    fn new<B1, B2>(low: B1, high: B2) -> Result<Self, rand::distr::uniform::Error>
    where
        B1: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self {
            x: C::Sampler::new(low.borrow().0.x, high.borrow().0.x)?,
            y: C::Sampler::new(low.borrow().0.y, high.borrow().0.y)?,
        })
    }

    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, rand::distr::uniform::Error>
    where
        B1: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
        B2: rand::distr::uniform::SampleBorrow<Self::X> + Sized,
    {
        Ok(Self {
            x: C::Sampler::new_inclusive(low.borrow().0.x, high.borrow().0.x)?,
            y: C::Sampler::new_inclusive(low.borrow().0.y, high.borrow().0.y)?,
        })
    }

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        SampleCoord(Coord {
            x: self.x.sample(rng),
            y: self.y.sample(rng),
        })
    }
}
