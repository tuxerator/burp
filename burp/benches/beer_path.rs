use std::{
    fs::File,
    io::Read,
    iter::{repeat_with, successors},
    path::PathBuf,
};

use burp::{
    graph::{oracle::Oracle, PoiGraph},
    types::Poi,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geo::{Coord, CoordNum};
use graph_rs::CoordGraph;
use memmap2::MmapOptions;
use rand::{
    distr::uniform::{SampleUniform, UniformSampler},
    Rng,
};
use rmp_serde::Deserializer;
use serde::Deserialize;

criterion_group!(query, beer_path_small, beer_path_big);
criterion_main!(query);

pub fn beer_path_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("beer-path small");

    let graph_file = File::open("../resources/small_poi.gmp").unwrap();
    let oracle_file = File::open("../resources/small_poi.omp").unwrap();

    let graph_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };
    let oracle_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };

    let mut graph_deser = Deserializer::from_read_ref(&graph_mmap);
    let mut oracle_deser = Deserializer::from_read_ref(&oracle_mmap);

    let graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut graph_deser).unwrap();
    let oracle: Oracle<f64> = Oracle::deserialize(&mut oracle_deser).unwrap();

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

    for size in size.take(10) {
        let s_t_pairs: Vec<(Coord, Coord)> = s_t_iter.take(size).collect();
        group.sample_size(100);
        group.throughput(Throughput::Elements(s_t_pairs.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("oracle", size),
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
            BenchmarkId::new("dijkstra", size),
            &s_t_pairs.clone(),
            |b, s_t_pairs| {
                b.iter(|| {
                    s_t_pairs.iter().for_each(|s_t| {
                        let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                        let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                        graph.beer_path_dijkstra_base(s_node, t_node, graph.poi_nodes());
                    });
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("dijkstra par", size),
            &s_t_pairs.clone(),
            |b, s_t_pairs| {
                b.iter(|| {
                    s_t_pairs.iter().for_each(|s_t| {
                        let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                        let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                        graph.beer_path_dijkstra_fast(s_node, t_node, graph.poi_nodes(), 0.2);
                    });
                });
            },
        );
    }
    group.finish();
}

pub fn beer_path_big(c: &mut Criterion) {
    let mut group = c.benchmark_group("beer-path big");

    let graph_file = File::open("../resources/medium_poi.gmp").unwrap();
    let oracle_file = File::open("../resources/medium_poi.omp").unwrap();

    let graph_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };
    let oracle_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };

    let mut graph_deser = Deserializer::from_read_ref(&graph_mmap);
    let mut oracle_deser = Deserializer::from_read_ref(&oracle_mmap);

    let graph: PoiGraph<Poi> = PoiGraph::deserialize(&mut graph_deser).unwrap();
    let oracle: Oracle<f64> = Oracle::deserialize(&mut oracle_deser).unwrap();

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

    for size in size.take(10) {
        let s_t_pairs: Vec<(Coord, Coord)> = s_t_iter.take(size).collect();
        group.sample_size(100);
        group.throughput(Throughput::Elements(s_t_pairs.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("oracle", size),
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
            BenchmarkId::new("dijkstra", size),
            &s_t_pairs.clone(),
            |b, s_t_pairs| {
                b.iter(|| {
                    s_t_pairs.iter().for_each(|s_t| {
                        let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                        let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                        graph.beer_path_dijkstra_base(s_node, t_node, graph.poi_nodes());
                    });
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("dijkstra par", size),
            &s_t_pairs.clone(),
            |b, s_t_pairs| {
                b.iter(|| {
                    s_t_pairs.iter().for_each(|s_t| {
                        let s_node = graph.graph().nearest_node(&s_t.0).unwrap();
                        let t_node = graph.graph().nearest_node(&s_t.1).unwrap();

                        graph.beer_path_dijkstra_fast(s_node, t_node, graph.poi_nodes(), 0.2);
                    });
                });
            },
        );
    }
    group.finish();
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
