use std::{
    cmp::max,
    collections::{HashSet, VecDeque},
    fmt::Debug,
    sync::{Arc, Weak},
};

use geo::{Coord, CoordFloat, Rect};
use graph_rs::{
    CoordGraph, Coordinate, DirectedGraph, Graph, algorithms::dijkstra::Dijkstra,
    graph::rstar::RTreeGraph, types::Direction,
};
use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, error, info, trace};
use num_traits::AsPrimitive;
use ordered_float::{FloatCore, OrderedFloat};
use rstar::{
    AABB, Envelope, RTree, RTreeNum, RTreeObject,
    primitives::{GeomWithData, Rectangle},
};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{types::RTreeObjectArc, util::r_tree_size};

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

    /// Returns the number of block-pairs stored in the oracle.
    pub fn size(&self) -> usize {
        self.block_pairs.len()
    }

    pub fn avg_block_ocupancy<G>(&self, graph: &RTreeGraph<G, C>) -> f64
    where
        G: DirectedGraph,
        G::NV: Coordinate<C>,
        G::EV: Default + Debug,
    {
        self.block_pairs
            .iter()
            .flat_map(|block_pair| {
                let s_ocupancy = graph.query(&block_pair.s_block.envelope()).count() as f64;
                let t_ocupancy = graph.query(&block_pair.t_block.envelope()).count() as f64;

                [s_ocupancy, t_ocupancy]
            })
            .enumerate()
            .reduce(|a, n| (n.0, a.1 + ((n.1 - a.1) / (n.0 as f64))))
            .unwrap()
            .1
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

        assert!(Arc::ptr_eq(
            &s_rect.data.upgrade().unwrap().s_block,
            &s_rect.geom().inner
        ));
        self.r_tree.insert(s_rect);
        self.r_tree.insert(t_rect);
        trace!("block paris length: {}", self.block_pairs.len());
        assert!(
            !self.block_pairs.contains(&block_pair),
            "block pair already exists\n{:#?}",
            &block_pair
        );
        self.block_pairs.push(block_pair.clone());
        trace!("Added block pair");

        block_pair
    }

    pub fn get_block_pairs(
        &self,
        s_coord: &Coord<C>,
        t_coord: &Coord<C>,
    ) -> Vec<Arc<BlockPair<C>>> {
        trace!(
            "Searching block pair for points\n{:#?}, {:#?}",
            s_coord, t_coord
        );
        let mut block_pairs: Vec<_> = self
            .r_tree
            .locate_all_at_point(s_coord)
            .filter_map(|geom| {
                if let Some(block_pair) = geom.data.upgrade() {
                    if block_pair.s_block.envelope().contains_point(s_coord)
                        && block_pair.t_block.envelope().contains_point(t_coord)
                    {
                        trace!("Found block pair {:#?}", block_pair);
                        return Some(block_pair);
                    }
                    return None;
                }
                None
            })
            .collect();

        block_pairs.dedup_by(|a, b| Arc::ptr_eq(a, b));
        block_pairs
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

    pub fn build_for_node<G>(&mut self, graph: &mut RTreeGraph<G, C>, node: &usize, epsilon: G::EV)
    where
        G::EV: FloatCore + Debug + Default,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph + Dijkstra,
    {
        debug!("Building oracle for node {:#?}", &node);
        let Some(root) = graph.bounding_rect() else {
            error!("Could not get bounding rect of graph");
            return;
        };

        let mut queue = VecDeque::new();

        queue.push_back((root, root));

        while let Some(block_pair) = queue.pop_front() {
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
                    panic!("Found empty block! This is a bug in the splitting operation");
                };

                trace!("A-Points: {:?}", points.0.count() + 1);
                trace!("B-Points: {:?}", points.1.count() + 1);

                s = p_0;
                t = p_1;
            }
            let Some(values) = Values::<G::EV>::new(graph, s, t, block_pair, *node) else {
                continue;
            };

            trace!("Epsilon: {:?}", &epsilon);
            trace!("Values:\n{:#?}", &values);

            if values.in_path(epsilon) {
                trace!("Found in-path block pair:\n{:#?}", &block_pair,);

                self.add_block_pair(block_pair.0, block_pair.1, *node);

                continue;
            } else if values.not_in_path(epsilon) {
                trace!("Found not in-path block pair:\n{:#?}", &block_pair,);

                continue;
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
                // .filter(|block_pair| {
                //     let points = (
                //         graph
                //             .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                //             .collect::<Vec<_>>(),
                //         graph
                //             .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                //             .collect::<Vec<_>>(),
                //     );
                //
                //     // If both blocks only contain the same node they can be discarded
                //     if points.0.len() == 1 && points.0 == points.1 {
                //         return false;
                //     }
                //     true
                // })
                .collect();

            queue.append(&mut child_block_pairs.into());
        }

        info!(
            "created oracle: {} block pairs, {} avg block ocupancy",
            self.size(),
            self.avg_block_ocupancy(graph)
        );
    }

    pub fn build_for_nodes<G>(
        &mut self,
        graph: &mut RTreeGraph<G, C>,
        nodes: &FxHashSet<usize>,
        epsilon: G::EV,
        progress_bar: Option<ProgressBar>,
    ) where
        G::EV: FloatCore + Debug + Default,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph + Dijkstra,
    {
        if let Some(pb) = progress_bar {
            pb.reset();
            pb.set_length(nodes.len().as_());
            pb.set_message("Building oracle for pois");

            nodes.iter().progress_with(pb).for_each(|point| {
                self.build_for_node(graph, point, epsilon);
            });
        } else {
            nodes.iter().for_each(|point| {
                self.build_for_node(graph, point, epsilon);
            });
        };
    }

    pub fn invariant<G>(&self, graph: &RTreeGraph<G, C>, node: &usize) -> bool
    where
        G::EV: FloatCore + Debug + Default,
        G::NV: Coordinate<C> + Debug,
        G: DirectedGraph,
    {
        let mut points = graph
            .nodes_iter()
            .flat_map(|p| graph.nodes_iter().map(move |q| (p, q)))
            .filter(|pair| pair.0.0 != pair.1.0);

        points.all(|point| {
            let block_pairs: Vec<_> = self
                .get_block_pairs(&point.0.1.as_coord(), &point.1.1.as_coord())
                .into_iter()
                .filter(|pair| pair.poi_id == *node)
                .collect();

            !block_pairs.len() > 1
        })
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

#[derive(Debug)]
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
        G: DirectedGraph + Dijkstra,
        G::NV: Coordinate<C>,
        G::EV: FloatCore + Debug + Default,
        C: RTreeNum + CoordFloat,
    {
        let d_s = graph.dijkstra(
            s.data,
            FxHashSet::from_iter([t.data, poi]),
            Direction::Outgoing,
        );
        Some(Values {
            d_st: *d_s.get(t.data)?.cost(),
            d_sp: *d_s.get(poi)?.cost(),
            d_pt: *graph
                .dijkstra(poi, FxHashSet::from_iter([t.data]), Direction::Outgoing)
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
        ((self.r_ab + self.d_sp + self.d_pt + self.r_bf)
            / max(
                OrderedFloat(self.d_st - (self.r_af + self.r_bb)),
                OrderedFloat(T::from(1).unwrap()),
            )
            .0)
            - T::from(1).unwrap()
            <= epsilon
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        ((self.d_sp + self.d_pt - (self.r_ab + self.r_bf)) / (self.d_st + (self.r_ab + self.r_bf)))
            - T::from(1).unwrap()
            >= epsilon
    }
}

#[cfg(test)]
mod test {
    use std::{f64, ops::Bound};

    use geo::{Coord, Rect};
    use graph_rs::graph::{csr::DirectedCsrGraph, rstar::RTreeGraph};
    use qutee::{Area, Boundary, Point};
    use rand::random;

    use super::Oracle;

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

        assert_eq!(block_pairs, vec![expected]);
    }
}
