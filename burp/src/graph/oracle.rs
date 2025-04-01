use std::{
    collections::{HashSet, VecDeque},
    fmt::Debug,
    hash::{Hash, RandomState},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, RwLock, Weak},
    time::Duration,
};

use deepsize::{known_deep_size, DeepSizeOf};
use geo::{Coord, CoordFloat, Rect};
use graph_rs::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    graph::{self, csr::DirectedCsrGraph, quad_tree::QuadGraph, rstar::RTreeGraph},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use indicatif::{MultiProgress, ProgressBar, ProgressIterator};
use log::{debug, error, info, trace, warn};
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

use crate::{types::RTreeObjectArc, util::r_tree_size};

use super::PoiGraph;

macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Some(x) => x,
            None => continue,
        }
    };
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct BlockPair<C>
where
    C: RTreeNum + CoordFloat,
{
    pub s_block: Arc<Rectangle<Coord<C>>>,
    pub t_block: Arc<Rectangle<Coord<C>>>,
    pub poi_id: usize,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Oracle<C = f64>
where
    C: RTreeNum + CoordFloat,
{
    r_tree: RTree<GeomWithData<RTreeObjectArc<Rectangle<Coord<C>>>, Weak<BlockPair<C>>>>,
    block_pairs: Vec<Arc<BlockPair<C>>>,
}

impl<C> Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    pub fn new() -> Self {
        debug!("BlockPair size: {}", size_of::<BlockPair<C>>());
        debug!("Rectangle size: {}", size_of::<Rectangle<Coord<C>>>());
        Oracle {
            r_tree: RTree::new(),
            block_pairs: vec![],
        }
    }

    pub fn r_tree_size(&self) -> usize {
        r_tree_size(self.r_tree.root())
    }

    fn add_block_pair(
        &mut self,
        s_block: Rect<C>,
        t_block: Rect<C>,
        poi: usize,
    ) -> Arc<BlockPair<C>> {
        let s_geom = Arc::new(Rectangle::from_corners(s_block.min(), s_block.max()));

        let t_geom = Arc::new(Rectangle::from_corners(t_block.min(), t_block.max()));

        let block_pair = Arc::new(BlockPair {
            s_block: s_geom.clone(),
            t_block: t_geom.clone(),
            poi_id: poi,
        });

        let s_rect = GeomWithData::new(RTreeObjectArc::new(s_geom), Arc::downgrade(&block_pair));
        let t_rect = GeomWithData::new(RTreeObjectArc::new(t_geom), Arc::downgrade(&block_pair));

        self.r_tree.insert(s_rect);
        self.r_tree.insert(t_rect);
        self.block_pairs.push(block_pair.clone());

        trace!("BlockPair value size: {}", size_of_val(&block_pair));
        debug!(
            "BlockPair vec size: {}",
            self.block_pairs.len() * size_of_val(&block_pair)
        );
        block_pair
    }

    pub fn get_block_pairs(
        &self,
        s_coord: &Coord<C>,
        t_coord: &Coord<C>,
    ) -> Vec<&Arc<BlockPair<C>>> {
        let s_blocks = self
            .r_tree
            .locate_all_at_point(s_coord)
            .filter_map(|geom| geom.data.upgrade());
        let t_blocks = self
            .r_tree
            .locate_all_at_point(t_coord)
            .filter_map(|geom| geom.data.upgrade());

        let s_blocks_ptr = FxHashSet::from_iter(s_blocks.map(|block| Arc::as_ptr(&block)));
        let t_blocks_ptr = FxHashSet::from_iter(t_blocks.map(|block| Arc::as_ptr(&block)));

        let block_pair_ptrs = FxHashSet::from_iter(s_blocks_ptr.intersection(&t_blocks_ptr));

        self.block_pairs
            .iter()
            .filter(|e| block_pair_ptrs.contains(&Arc::as_ptr(e)))
            .collect()
    }

    pub fn get_pois(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> HashSet<usize> {
        let block_pairs = self.get_block_pairs(s_coord, t_coord);

        block_pairs.into_iter().map(|b| b.poi_id).collect()
    }

    pub fn get_blocks_at(&self, coord: &Coord<C>) -> Vec<Arc<BlockPair<C>>> {
        self.r_tree
            .locate_all_at_point(coord)
            .filter_map(|geom| geom.data.upgrade())
            .collect()
    }

    pub fn build_for_node<G>(&mut self, graph: &mut RTreeGraph<G, C>, node: usize, epsilon: G::EV)
    where
        G::EV: FloatCore,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph + CachedDijkstra,
    {
        let Some(root) = graph.bounding_rect() else {
            error!("Could not get bounding rect of graph");
            return;
        };

        let mut queue = VecDeque::new();

        queue.push_back((root, root));

        while let Some(block_pair) = queue.pop_front() {
            if block_pair.0 != block_pair.1 {
                let s: GeomWithData<Coord<C>, usize>;
                let t: GeomWithData<Coord<C>, usize>;

                {
                    let mut points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .cloned(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .cloned(),
                    );

                    let (Some(p_0), Some(p_1)) = (points.0.next(), points.1.next()) else {
                        trace!("At least one block is empty. {:?}", &block_pair);
                        continue;
                    };

                    s = p_0;
                    t = p_1;
                }
                let Some(values) = Values::<G::EV>::new(graph, s, t, block_pair, node) else {
                    continue;
                };

                if values.in_path(epsilon) {
                    trace!("Found in-path block pair: {:#?}", &block_pair);

                    self.add_block_pair(block_pair.0, block_pair.1, node);

                    continue;
                } else if values.not_in_path(epsilon) {
                    trace!("Found not in-path block pair: {:#?}", &block_pair);

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

            queue.append(&mut child_block_pairs.into());
        }
    }

    pub fn build_for_nodes<G>(
        &mut self,
        graph: &mut RTreeGraph<G, C>,
        nodes: &FxHashSet<usize>,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) where
        G::EV: FloatCore,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph + CachedDijkstra,
    {
        if let Some(pb) = progress_bar {
            pb.reset();
            pb.set_length(nodes.len().as_());
            pb.set_message("Building oracle for pois");

            nodes.iter().progress_with(pb).for_each(|point| {
                self.build_for_node(graph, *point, epsilon);
            });
        } else {
            nodes.iter().for_each(|point| {
                self.build_for_node(graph, *point, epsilon);
            });
        };
        info!(
            "created oracle containing {} block pair",
            self.r_tree.size()
        );
    }
}

impl<C> Oracle<C>
where
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
    use graph_rs::graph::{csr::DirectedCsrGraph, rstar::RTreeGraph};
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
        let mut oracle = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        oracle.add_block_pair(rect_0, rect_1, 0);
    }

    #[test]
    fn get_block_pairs_test() {
        let mut oracle = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        let expected = oracle.add_block_pair(rect_0, rect_1, 0);

        let block_pairs = oracle.get_block_pairs(&(0.6, 0.8).into(), &(10.5, 10.).into());

        assert_eq!(block_pairs, vec![&expected]);
    }
}
