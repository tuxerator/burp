use std::{
    collections::{HashSet, VecDeque},
    fmt::Debug,
    hash::{Hash, RandomState},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use geo::{Coord, CoordFloat, Rect};
use graph_rs::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph, rstar::RTreeGraph},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use indicatif::{MultiProgress, ProgressBar, ProgressIterator};
use log::{debug, error, info, warn};
use num_traits::{pow, AsPrimitive, Num};
use ordered_float::FloatCore;
use qutee::{Boundary, DynCap, Point, QueryPoints};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rstar::{
    primitives::{GeomWithData, ObjectRef, Rectangle},
    RTree, RTreeNum, RTreeObject, RTreeParams, AABB,
};
use rustc_hash::FxHashSet;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Some(x) => x,
            None => continue,
        }
    };
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BlockPair<C>
where
    C: RTreeNum + CoordFloat,
{
    pub s_block: Rectangle<Coord<C>>,
    pub t_block: Rectangle<Coord<C>>,
    pub poi_id: usize,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Oracle<G, C = f64>
where
    G: DirectedGraph + CachedDijkstra,
    G::EV: FloatCore,
    G::NV: Coordinate<C> + Debug,
    C: RTreeNum + CoordFloat,
{
    graph: Arc<RwLock<RTreeGraph<G, C>>>,
    r_tree: RTree<GeomWithData<Rectangle<Coord<C>>, usize>>,
    block_pairs: Vec<BlockPair<C>>,
}

impl<G, C> Oracle<G, C>
where
    G: DirectedGraph + CachedDijkstra + Send + Sync,
    G::EV: FloatCore + Send + Sync,
    G::NV: Coordinate<C> + Debug + Send + Sync,
    C: RTreeNum + CoordFloat + Send + Sync,
{
    pub fn new(graph: Arc<RwLock<RTreeGraph<G, C>>>) -> Self {
        Oracle {
            graph,
            r_tree: RTree::new(),
            block_pairs: vec![],
        }
    }

    pub fn get_graph_ref(&self) -> Arc<RwLock<RTreeGraph<G, C>>> {
        self.graph.clone()
    }

    fn add_block_pair(&mut self, s_block: Rect<C>, t_block: Rect<C>, poi: usize) -> BlockPair<C> {
        let s_geom = Rectangle::from_corners(s_block.min(), s_block.max());

        let t_geom = Rectangle::from_corners(t_block.min(), t_block.max());

        let block_pair = BlockPair {
            s_block: s_geom,
            t_block: t_geom,
            poi_id: poi,
        };

        self.block_pairs.push(block_pair);

        let block_pair = self.block_pairs.last().expect("block_pairs is empty");

        let s_rect = GeomWithData::new(block_pair.s_block, self.block_pairs.len() - 1);
        let t_rect = GeomWithData::new(block_pair.t_block, self.block_pairs.len() - 1);

        self.r_tree.insert(s_rect);
        self.r_tree.insert(t_rect);

        *block_pair
    }

    pub fn get_block_pairs(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> Vec<&BlockPair<C>> {
        let s_blocks = self
            .r_tree
            .locate_all_at_point(s_coord)
            .map(|geom| geom.data);
        let t_blocks = self
            .r_tree
            .locate_all_at_point(t_coord)
            .map(|geom| geom.data);

        let s_blocks = HashSet::<_, RandomState>::from_iter(s_blocks);
        let t_blocks = HashSet::<_, RandomState>::from_iter(t_blocks);

        let block_pair_ids = HashSet::<_, RandomState>::from_iter(s_blocks.intersection(&t_blocks));

        self.block_pairs
            .iter()
            .enumerate()
            .filter(|(i, _)| block_pair_ids.contains(i))
            .map(|e| e.1)
            .collect()
    }

    pub fn get_pois(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> HashSet<usize> {
        let block_pairs = self.get_block_pairs(s_coord, t_coord);

        block_pairs.into_iter().map(|b| b.poi_id).collect()
    }

    pub fn get_blocks_at(&self, coord: &Coord<C>) -> Vec<&BlockPair<C>> {
        let block_pairs = self.r_tree.locate_all_at_point(coord).map(|geom| geom.data);

        let block_pairs = HashSet::<_, RandomState>::from_iter(block_pairs);

        self.block_pairs
            .iter()
            .enumerate()
            .filter(|(i, _)| block_pairs.contains(i))
            .map(|e| e.1)
            .collect()
    }

    pub fn build_for_node(
        &mut self,
        point: usize,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) {
        let oracle = Arc::new(RwLock::new(self));

        Self::build_for_node_par(oracle, point, epsilon, progress_bar);
    }

    fn build_for_node_par(
        oracle: Arc<RwLock<&mut Self>>,
        node: usize,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) where
        G::EV: FloatCore,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph + CachedDijkstra,
    {
        let oracle_ref = oracle.read().expect("RwLock poisoned");
        let graph = oracle_ref.graph.read().expect("RwLock poisoned");

        let Some(root) = graph.bounding_rect() else {
            error!("Could not get bounding rect of graph");
            return;
        };

        drop(graph);
        drop(oracle_ref);

        let mut queue = VecDeque::new();

        queue.push_back((root, root));

        while let Some(block_pair) = queue.pop_front() {
            if block_pair.0 != block_pair.1 {
                let s: GeomWithData<Coord<C>, usize>;
                let t: GeomWithData<Coord<C>, usize>;

                let oracle_ref = oracle.read().expect("RwLock poisoned");
                {
                    let graph = oracle_ref.graph.read().expect("RwLock poisoned");
                    let mut points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .cloned(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .cloned(),
                    );

                    let size = (points.0.size_hint(), points.1.size_hint());

                    let (Some(p_0), Some(p_1)) = (points.0.next(), points.1.next()) else {
                        debug!("At least one block is empty. {:?}", &block_pair);
                        continue;
                    };

                    s = p_0;
                    t = p_1;
                }
                let Some(values) = Values::<G::EV>::new(
                    &mut oracle_ref.graph.write().expect("RwLock poisoned"),
                    s,
                    t,
                    block_pair,
                    node,
                ) else {
                    continue;
                };

                drop(oracle_ref);

                if values.in_path(epsilon) {
                    debug!("Found in-path block pair: {:#?}", &block_pair);

                    oracle.write().expect("Mutex poisoned").add_block_pair(
                        block_pair.0,
                        block_pair.1,
                        node,
                    );

                    continue;
                } else if values.not_in_path(epsilon) {
                    debug!("Found not in-path block pair: {:#?}", &block_pair);

                    continue;
                }
            }

            let children = (
                block_pair
                    .0
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
                block_pair
                    .1
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
            );

            let oracle_ref = oracle.read().expect("RwLock poisoned");

            let graph = oracle_ref.graph.read().expect("RwLock poisoned");
            let children = (
                children
                    .0
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
                children
                    .1
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
            );
            let child_block_pairs: Vec<_> = children
                .0
                .into_iter()
                .flat_map(|block_a| children.1.iter().map(move |block_b| (block_a, *block_b)))
                .filter(|block_pair| {
                    let points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .collect::<Vec<_>>(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .collect::<Vec<_>>(),
                    );

                    // If both blocks only contain the same node they can be discarded
                    if points.0.len() == 1 && points.0 == points.1 {
                        return false;
                    }
                    true
                })
                .collect();

            debug!("inserting {} block pairs", child_block_pairs.len());

            queue.append(&mut child_block_pairs.into());
        }
    }

    pub fn build_for_points(
        &mut self,
        points: &FxHashSet<usize>,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) {
        if let Some(pb) = progress_bar {
            pb.reset();
            pb.set_length(points.len().as_());
            pb.set_message("Building oracle for pois");

            points.iter().progress_with(pb).for_each(|point| {
                self.build_for_node(*point, epsilon, None);
            });
        } else {
            points.iter().for_each(|point| {
                self.build_for_node(*point, epsilon, None);
            });
        };
        info!(
            "created oracle containing {} block pair",
            self.r_tree.size()
        );
    }

    pub fn build_for_points_par(
        &mut self,
        points: &FxHashSet<usize>,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) {
        let self_ref = Arc::new(RwLock::new(self));

        if let Some(pb_outer) = progress_bar {
            pb_outer.reset();
            pb_outer.set_length(points.len().as_());

            points.into_par_iter().for_each(|point| {
                Self::build_for_node_par(self_ref.clone(), *point, epsilon, None);
                pb_outer.inc(1);
            });

            pb_outer.finish_and_clear();
        } else {
            points.into_par_iter().for_each(|point| {
                Self::build_for_node_par(self_ref.clone(), *point, epsilon, None);
            });
        };
        info!(
            "created oracle containing {} block pair",
            self_ref.read().expect("RwLock poisoned").r_tree.size()
        );
    }
}

impl<G, C> Oracle<G, C>
where
    G: DirectedGraph + CachedDijkstra + Send + Sync + Serialize + DeserializeOwned,
    G::EV: FloatCore + Send + Sync,
    G::NV: Coordinate<C> + Debug + Send + Sync,
    C: RTreeNum + CoordFloat + Send + Sync + Serialize + DeserializeOwned,
{
    pub fn to_flexbuffer(&self) -> Vec<u8> {
        let mut ser = flexbuffers::FlexbufferSerializer::new();

        info!("serializing oracle to flexbuffer");
        self.serialize(&mut ser).unwrap();

        ser.take_buffer()
    }

    pub fn read_flexbuffer(f_buf: &[u8]) -> Self {
        let reader = flexbuffers::Reader::get_root(f_buf).unwrap();

        Self::deserialize(reader).unwrap()
    }
}

struct Values<T: FloatCore> {
    d_st: T,
    d_sp: T,
    d_pt: T,
    r_af: T,
    r_ab: T,
    r_bf: T,
    r_bb: T,
}

impl<T: FloatCore> Values<T> {
    fn new<G, C>(
        graph: &mut RTreeGraph<G, C>,
        s: GeomWithData<Coord<C>, usize>,
        t: GeomWithData<Coord<C>, usize>,
        block_pair: (Rect<C>, Rect<C>),
        poi: usize,
    ) -> Option<Values<G::EV>>
    where
        G: DirectedGraph + CachedDijkstra,
        G::NV: Coordinate<C>,
        G::EV: FloatCore,
        C: RTreeNum + CoordFloat,
    {
        Some(Values {
            d_st: *graph
                .cached_dijkstra(s.data, HashSet::from([t.data]), Direction::Outgoing)
                .unwrap()
                .get(t.data)?
                .cost(),
            d_sp: *graph
                .cached_dijkstra(s.data, HashSet::from([poi]), Direction::Outgoing)
                .unwrap()
                .get(poi)?
                .cost(),
            d_pt: *graph
                .cached_dijkstra(poi, HashSet::from([t.data]), Direction::Outgoing)
                .unwrap()
                .get(t.data)?
                .cost(),
            r_af: graph
                .radius(
                    s.data,
                    &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                    Direction::Outgoing,
                )
                .unwrap(),
            r_ab: graph
                .radius(
                    s.data,
                    &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                    Direction::Incoming,
                )
                .unwrap(),
            r_bf: graph
                .radius(
                    t.data,
                    &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                    Direction::Outgoing,
                )
                .unwrap(),
            r_bb: graph
                .radius(
                    t.data,
                    &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                    Direction::Incoming,
                )
                .unwrap(),
        })
    }

    fn in_path(&self, epsilon: T) -> bool {
        (self.r_ab + self.d_sp + self.d_pt + self.r_bf)
            <= (self.d_st - (self.r_af + self.r_bb)) * (T::from(1).unwrap() + epsilon)
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt - (self.r_af + self.r_bb))
            >= (self.d_st + (self.r_ab + self.r_bf)) * (T::from(1).unwrap() + epsilon)
    }
}

fn divide<C: qutee::Coordinate>(block: &Boundary<C>) -> [Boundary<C>; 4] {
    let half_block_height = block.height()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let half_block_width = block.width()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let minus_one = C::from(-1).unwrap_or_else(|| {
        panic!(
            "Could not convert '-1' into type '{}'",
            std::any::type_name::<C>()
        )
    });
    let top_left = block.top_left();
    let corner_points = [
        *top_left,
        Point::new(top_left.x + half_block_width, top_left.y),
        Point::new(top_left.x, top_left.y - half_block_height),
        Point::new(
            top_left.x + half_block_width,
            top_left.y - half_block_height,
        ),
    ];

    corner_points
        .into_iter()
        .map(|corner| Boundary::new(corner, half_block_width, half_block_height * minus_one))
        .collect::<Vec<Boundary<C>>>()
        .try_into()
        .unwrap_or_else(|_| {
            panic!(
                "Could not convert 'Vec<Boundary<{}> to '[Boundary<{}>; 4]'",
                std::any::type_name::<C>(),
                std::any::type_name::<C>()
            )
        })
}

#[cfg(test)]
mod test {
    use std::{f64, ops::Bound};

    use geo::{Coord, Rect};
    use graph_rs::graph::csr::DirectedCsrGraph;
    use qutee::{Area, Boundary, Point};
    use rand::random;

    use super::{divide, BlockPair, Oracle};

    #[test]
    fn corner_points_test() {
        for _ in 0..1000 {
            let point: Point<f64> = Point::new(
                random::<f64>() * 360.0 - 180.0,
                random::<f64>() * 180.0 - 90.0,
            );
            let block = Boundary::new(point, random::<f64>() * 360.0, -random::<f64>() * 180.0);
            let split = divide(&block);
            let expected = [
                Boundary::new(point, block.width() / 2.0, -block.height() / 2.0),
                Boundary::new(
                    Point::new(point.x + block.width() / 2.0, point.y),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(point.x, point.y - block.height() / 2.0),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(
                        point.x + block.width() / 2.0,
                        point.y - block.height() / 2.0,
                    ),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
            ];

            assert_eq!(split, expected);
            for i in 0..4 {
                for j in 0..4 {
                    if i != j {
                        assert!(!split[i].intersects(&split[j]));
                    }
                }
            }
        }
    }

    #[test]
    fn add_block_pair_test() {
        let mut oracle: Oracle<DirectedCsrGraph<f64, Coord<f64>>> = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        oracle.add_block_pair(rect_0, rect_1, 0);
    }

    #[test]
    fn get_block_pairs_test() {
        let mut oracle: Oracle<DirectedCsrGraph<f64, Coord<f64>>> = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        let expected = oracle.add_block_pair(rect_0, rect_1, 0);

        let block_pairs = oracle.get_block_pairs(&(0.6, 0.8).into(), &(10.5, 10.).into());

        assert_eq!(block_pairs, vec![&expected]);
    }
}
