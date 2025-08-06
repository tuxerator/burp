use std::{
    cmp::max,
    collections::{HashSet, VecDeque},
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{Arc, Weak},
};

use geo::{Contains, Coord, CoordFloat, Rect};
use graph_rs::{
    CoordGraph, Coordinate, DirectedGraph, Graph,
    algorithms::dijkstra::{Dijkstra, ResultNode},
    graph::{self, Path, rstar::RTreeGraph},
    types::Direction,
};
use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, error, info, trace};
use num_traits::{AsPrimitive, Num, NumCast};
use ordered_float::{FloatCore, OrderedFloat};
use rstar::{
    AABB, Envelope, RTree, RTreeNum, RTreeObject,
    primitives::{GeomWithData, Rectangle},
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeStruct};
use tracing::instrument;

use crate::{
    tree::{Tree, node::Node},
    types::RTreeObjectArc,
    util::r_tree_size,
};

use super::block_pair::BlockPair;

pub trait Radius: CoordGraph {
    fn radius(
        &self,
        node: usize,
        envelope: &Rect<Self::C>,
        direction: Direction,
    ) -> Option<Path<Self::EV>>;
}

impl<T> Radius for T
where
    T: CoordGraph + Dijkstra,
    T::EV: FloatCore + Debug,
{
    #[instrument(skip(self))]
    fn radius(
        &self,
        node: usize,
        envelope: &Rect<Self::C>,
        direction: Direction,
    ) -> Option<Path<Self::EV>> {
        let nodes = self.locate_in_envelope(envelope);
        let nodes = HashSet::from_iter(nodes);

        if !nodes.contains(&node) {
            info!("Node not found");
            return None;
        }

        let distances = self.dijkstra(node, nodes.clone(), direction);
        let max = distances
            .0
            .iter()
            .filter(|e| nodes.contains(&e.node_id()))
            .max_by(|rhs, lhs| OrderedFloat(*rhs.cost()).cmp(&OrderedFloat(*lhs.cost())))?
            .node_id();

        distances.path(max)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Oracle<EV, C>
where
    EV: FloatCore,
    C: RTreeNum + CoordFloat,
{
    r_tree: RTree<GeomWithData<Rectangle<Coord<C>>, Weak<BlockPair<EV, C>>>>,

    block_pairs: Vec<Arc<BlockPair<EV, C>>>,
}

impl<EV, C> Oracle<EV, C>
where
    EV: FloatCore + Debug,
    C: RTreeNum + CoordFloat,
{
    pub fn new() -> Self {
        Oracle {
            r_tree: RTree::new(),
            block_pairs: vec![],
        }
    }

    /// Returns the number of block-pairs stored in the oracle.
    pub fn size(&self) -> usize {
        self.block_pairs.len()
    }

    pub fn avg_block_ocupancy<G>(&self, graph: &G) -> f64
    where
        G: CoordGraph<C = C, EV = EV>,
    {
        self.block_pairs
            .iter()
            .flat_map(|block_pair| {
                let s_ocupancy = graph.locate_in_envelope(block_pair.s_block()).count() as f64;
                let t_ocupancy = graph.locate_in_envelope(block_pair.t_block()).count() as f64;

                [s_ocupancy, t_ocupancy]
            })
            .enumerate()
            .reduce(|a, n| (n.0, a.1 + ((n.1 - a.1) / (n.0 as f64))))
            .unwrap()
            .1
    }

    #[instrument(skip(self))]
    fn add_block_pair(&mut self, block_pair: BlockPair<EV, C>) -> Arc<BlockPair<EV, C>> {
        let block_pair = Arc::new(block_pair);

        let s_rect = GeomWithData::new(
            block_pair.s_block_as_rectangle(),
            Arc::downgrade(&block_pair),
        );
        let t_rect = GeomWithData::new(
            block_pair.t_block_as_rectangle(),
            Arc::downgrade(&block_pair),
        );

        assert_eq!(
            &s_rect.data.upgrade().unwrap().s_block_as_rectangle(),
            s_rect.geom()
        );
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
    ) -> Vec<Arc<BlockPair<EV, C>>> {
        trace!(
            "Searching block pair for points\n{:#?}, {:#?}",
            s_coord, t_coord
        );
        let mut block_pairs: Vec<_> = self
            .r_tree
            .locate_all_at_point(s_coord)
            .filter_map(|geom| {
                if let Some(block_pair) = geom.data.upgrade() {
                    if block_pair.s_block().contains(s_coord)
                        && block_pair.t_block().contains(t_coord)
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

    pub fn get_beer_pois(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> HashSet<usize> {
        let block_pairs = self.get_block_pairs(s_coord, t_coord);

        block_pairs.into_iter().map(|b| b.poi_id()).collect()
    }

    pub fn get_blocks_at(&self, coord: &Coord<C>) -> Vec<Arc<BlockPair<EV, C>>> {
        self.r_tree
            .locate_all_at_point(coord)
            .filter_map(|geom| geom.data.upgrade())
            .collect()
    }

    #[instrument(skip(self, graph))]
    pub fn build_for_node<G>(
        &mut self,
        node: usize,
        epsilon: G::EV,
        graph: &G,
    ) -> Result<Tree<BlockPair<G::EV, G::C>>, String>
    where
        G: CoordGraph<C = C, EV = EV> + Dijkstra + Radius,
    {
        debug!("Building oracle for node {:#?}", &node);
        let Some(root) = graph.bounding_rect() else {
            return Err("Could not get bounding rect of graph".to_string());
        };

        let mut queue = VecDeque::new();

        let root = BlockPair::new(root, root, node, graph);

        let mut tree = Tree::new(Node::new(root.clone(), None));

        let node_ptr = &raw mut *tree.get_root_mut();

        queue.push_back((root, node_ptr));

        while let Some((block_pair, node_ptr)) = queue.pop_front() {
            // let Some(values) = Values::<G::EV>::new(graph, s, t, block_pair, *node) else {
            //     continue;
            // };

            trace!("Epsilon: {:?}", &epsilon);
            trace!("Values:\n{:#?}", block_pair.values());

            if block_pair.values().in_path(epsilon) {
                trace!("Found in-path block pair:\n{:#?}", &block_pair,);

                self.add_block_pair(block_pair.clone());

                continue;
            } else if block_pair.values().not_in_path(epsilon) {
                trace!("Found not in-path block pair:\n{:#?}", &block_pair,);

                continue;
            }

            let children = (
                block_pair
                    .s_block()
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
                block_pair
                    .t_block()
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
            );

            let children = (
                children
                    .0
                    .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                    .collect::<Vec<_>>(),
                children
                    .1
                    .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                    .collect::<Vec<_>>(),
            );
            let child_block_pairs: Vec<_> = children
                .0
                .into_iter()
                .flat_map(|s_block| {
                    children.1.iter().map(move |t_block| {
                        let block_pair = BlockPair::new(s_block, *t_block, node, graph);
                        let child_ptr;

                        debug!("node_ptr: {:#?}", node_ptr);

                        // SAFETY: Tree lives in this function scope and the children vector has
                        // capacity 16.
                        unsafe {
                            let node = &mut *node_ptr;

                            child_ptr = &raw mut *node.insert_child(block_pair.clone());
                        }

                        debug!("child_ptr: {child_ptr:?}");

                        (block_pair, child_ptr)
                    })
                })
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

            // debug!("tree: {tree:#?}");
        }

        info!(
            "created oracle: {} block pairs, {} avg block ocupancy",
            self.size(),
            self.avg_block_ocupancy(graph)
        );

        Ok(tree)
    }

    pub fn invariant<G>(&self, node: usize, graph: &G) -> bool
    where
        G: CoordGraph<C = C>,
        G::NV: Coordinate<G::C>,
    {
        let mut points = graph
            .nodes_iter()
            .flat_map(|p| graph.nodes_iter().map(move |q| (p, q)))
            .filter(|pair| pair.0.0 != pair.1.0);

        points.all(|point| {
            let block_pairs: Vec<_> = self
                .get_block_pairs(&point.0.1.as_coord(), &point.1.1.as_coord())
                .into_iter()
                .filter(|pair| pair.poi_id() == node)
                .collect();

            !block_pairs.len() > 1
        })
    }
}

// impl<C> Oracle<C>
// where
//     C: RTreeNum + CoordFloat + Send + Sync + Serialize + DeserializeOwned,
// {
//     pub fn to_flexbuffer(&self) -> Vec<u8> {
//         let mut ser = flexbuffers::FlexbufferSerializer::new();
//
//         info!("serializing oracle to flexbuffer");
//         self.serialize(&mut ser).unwrap();
//
//         ser.take_buffer()
//     }
//
//     pub fn read_flexbuffer(f_buf: &[u8]) -> Self {
//         let reader = flexbuffers::Reader::get_root(f_buf).unwrap();
//
//         Self::deserialize(reader).unwrap()
//     }
// }

#[derive(Default)]
pub struct OracleCollection<G>
where
    G: CoordGraph,
    G::NV: Coordinate<G::C>,
    G::EV: FloatCore,
    G::C: RTreeNum + CoordFloat,
{
    oracle: FxHashMap<usize, Oracle<G::EV, G::C>>,
    phantom: PhantomData<G>,
}

impl<G> OracleCollection<G>
where
    G: CoordGraph + Dijkstra + Radius,
    G::NV: Coordinate<G::C>,
    G::EV: FloatCore + Debug,
    G::C: RTreeNum + CoordFloat,
{
    pub fn build_for_node(
        &mut self,
        node: usize,
        epsilon: G::EV,
        graph: &G,
    ) -> Result<Tree<BlockPair<G::EV, G::C>>, String> {
        let mut oracle = Oracle::new();

        let debug_tree = oracle.build_for_node(node, epsilon, graph)?;

        self.oracle.insert(node, oracle);
        Ok(debug_tree)
    }

    pub fn build_for_nodes(
        &mut self,
        nodes: &Vec<usize>,
        epsilon: G::EV,
        graph: &G,
    ) -> Result<Vec<Tree<BlockPair<G::EV, G::C>>>, String> {
        let mut debug_trees = Vec::new();
        for node in nodes {
            debug_trees.push(self.build_for_node(*node, epsilon, graph)?);
        }

        Ok(debug_trees)
    }
}

#[cfg(test)]
mod test {
    use std::{f64, ops::Bound};

    use geo::{Coord, Rect};
    use graph_rs::{
        DirectedGraph, Graph,
        graph::{csr::DirectedCsrGraph, rstar::RTreeGraph},
    };
    use qutee::{Area, Boundary, Point};
    use rand::random;
    use serde::{Deserialize, Serialize};

    use crate::oracle::block_pair::BlockPair;

    use super::Oracle;

    #[test]
    fn add_block_pair_test() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord<f64>>, f64> =
            RTreeGraph::new_from_graph(DirectedCsrGraph::default());
        let mut oracle = Oracle::new();

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            &graph,
        );

        oracle.add_block_pair(block_pair);
    }

    #[test]
    fn get_block_pairs_test() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord<f64>>, f64> =
            RTreeGraph::new_from_graph(DirectedCsrGraph::default());
        let mut oracle = Oracle::new();

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            &graph,
        );

        let expected = oracle.add_block_pair(block_pair);

        let block_pairs = oracle.get_block_pairs(&(0.6, 0.8).into(), &(10.5, 10.).into());

        assert_eq!(block_pairs, vec![expected]);
    }

    #[test]
    fn ser_de() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord<f64>>, f64> =
            RTreeGraph::new_from_graph(DirectedCsrGraph::default());
        let mut oracle = Oracle::new();

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            &graph,
        );

        oracle.add_block_pair(block_pair);

        let mut buf = vec![];

        oracle
            .serialize(&mut rmp_serde::Serializer::new(&mut buf))
            .unwrap();

        let oracle_de: Oracle<f64, f64> =
            Oracle::deserialize(&mut rmp_serde::Deserializer::new(buf.as_slice())).unwrap();
    }
}
