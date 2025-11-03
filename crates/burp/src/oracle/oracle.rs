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
use tracing_subscriber::filter::combinator::Or;

use crate::{
    oracle::{OracleParams, SplitStrategy, block_pair, split_strategy::SimpleSplitStrategy},
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
    #[instrument(level = "trace", skip(self))]
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
    poi: usize,

    r_tree: RTree<GeomWithData<Rectangle<Coord<C>>, Weak<BlockPair<EV, C>>>>,

    block_pairs: Vec<Arc<BlockPair<EV, C>>>,
}

impl<EV, C> Oracle<EV, C>
where
    EV: FloatCore + Debug,
    C: RTreeNum + CoordFloat,
{
    pub fn new(poi: usize) -> Self {
        Oracle {
            poi,
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

    #[instrument(level = "trace", skip(self))]
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

    #[instrument(skip(graph))]
    pub fn build_for_node<G, P>(
        node: usize,
        epsilon: G::EV,
        graph: &G,
        params: P,
    ) -> Result<(Self, id_tree::Tree<(BlockPair<G::EV, G::C>, bool)>), String>
    where
        G: CoordGraph<C = C, EV = EV> + Dijkstra + Radius,
        P: OracleParams,
    {
        let mut oracle = Oracle::new(node);
        debug!("Building oracle for node {:#?}", &node);
        let Some(root) = graph.bounding_rect() else {
            return Err("Could not get bounding rect of graph".to_string());
        };

        let root = BlockPair::new(root, root, node, epsilon, graph);

        let mut tree = id_tree::TreeBuilder::new()
            .with_node_capacity(100000)
            .build();

        let root = tree
            .insert(
                id_tree::Node::new((root, false)),
                id_tree::InsertBehavior::AsRoot,
            )
            .unwrap();

        oracle.process_block_pair(&root, &mut tree, graph, params);

        Ok((oracle, tree))
    }

    /// Process the block pair in 'node'.
    ///
    /// Returns 1 if it is in-path, -1 if not-in-path and 0 if neither.
    fn process_block_pair<G, P>(
        &mut self,
        node: &id_tree::NodeId,
        tree: &mut id_tree::Tree<(BlockPair<EV, C>, bool)>,
        graph: &G,
        params: P,
    ) -> i32
    where
        G: CoordGraph<C = C, EV = EV> + Dijkstra + Radius,
        P: OracleParams,
    {
        let block_pair = &tree.get(node).unwrap().data().0;

        tracing::trace!(tree_capacity = ?tree.capacity());

        if block_pair.values().in_path() {
            log::trace!("Found in-path block pair:\n{:#?}", block_pair,);

            tree.get_mut(node).unwrap().data_mut().1 = true;

            return 1;
        }

        if block_pair.values().not_in_path() {
            log::trace!("Found not in-path block pair:\n{:#?}", block_pair,);

            if params.merge_blocks() {
                let _ = tree
                    .remove_node(node.clone(), id_tree::RemoveBehavior::DropChildren)
                    .inspect_err(|e| tracing::error!("Coud not remove node. Reason: {e}"));
            }

            return -1;
        }

        let children = P::SplitStrategy::split(block_pair, graph);

        let children_ids: Vec<_> = children
            .into_iter()
            .map(|child| {
                tree.insert(
                    id_tree::Node::new((child, false)),
                    id_tree::InsertBehavior::UnderNode(node),
                )
                .unwrap()
            })
            .collect();

        let children_in_path: Vec<_> = children_ids
            .iter()
            .map(|child| self.process_block_pair(child, tree, graph, params))
            .collect();

        if children_in_path.iter().all(|in_path| *in_path == 1) && params.merge_blocks() {
            log::trace!("All children are in-path");
            tree.get_mut(node).unwrap().data_mut().1 = true;

            for child in children_ids {
                let _ = tree
                    .remove_node(child, id_tree::RemoveBehavior::DropChildren)
                    .inspect_err(|e| tracing::error!("Could not remove node. Reason: {e}"));
            }

            return 1;
        }

        if children_in_path.iter().all(|in_path| *in_path == -1) && params.merge_blocks() {
            log::trace!("All children are not-in-path");

            let _ = tree
                .remove_node(node.clone(), id_tree::RemoveBehavior::DropChildren)
                .inspect_err(|e| tracing::error!("Could not remove node. Reason: {e}"));

            return -1;
        }

        if let Ok(children) = tree.children(node) {
            for child in children {
                if child.data().1 {
                    self.add_block_pair(child.data().0.clone());
                }
            }
        }

        0
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

    pub fn poi(&self) -> usize {
        self.poi
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

#[derive(Default, Serialize, Deserialize)]
pub struct OracleCollection<G>
where
    G: CoordGraph,
    G::NV: Coordinate<G::C>,
    G::EV: FloatCore + Serialize + DeserializeOwned,
    G::C: RTreeNum + CoordFloat + Serialize + DeserializeOwned,
{
    oracle: FxHashMap<usize, Oracle<G::EV, G::C>>,
    phantom: PhantomData<G>,
}

impl<G> OracleCollection<G>
where
    G: CoordGraph + Dijkstra + Radius,
    G::NV: Coordinate<G::C>,
    G::EV: FloatCore + Debug + Serialize + DeserializeOwned,
    G::C: RTreeNum + CoordFloat + Serialize + DeserializeOwned,
{
    pub fn build_for_node<P: OracleParams>(
        &mut self,
        node: usize,
        epsilon: G::EV,
        graph: &G,
        params: P,
    ) -> Result<(usize, id_tree::Tree<(BlockPair<G::EV, G::C>, bool)>), String> {
        let oracle = Oracle::build_for_node(node, epsilon, graph, params)?;

        self.oracle.insert(node, oracle.0);
        Ok((node, oracle.1))
    }

    pub fn build_for_nodes<P: OracleParams>(
        &mut self,
        nodes: &FxHashSet<usize>,
        epsilon: G::EV,
        graph: &G,
        params: P,
    ) -> Result<FxHashMap<usize, id_tree::Tree<(BlockPair<G::EV, G::C>, bool)>>, String> {
        let mut split_trees = FxHashMap::default();
        for node in nodes {
            let split_tree = self.build_for_node(*node, epsilon, graph, params)?;
            split_trees.insert(split_tree.0, split_tree.1);
        }

        Ok(split_trees)
    }

    pub fn insert(&mut self, oracle: Oracle<G::EV, G::C>) -> Option<Oracle<G::EV, G::C>> {
        self.oracle.insert(oracle.poi(), oracle)
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&Oracle<G::EV, G::C>>
    where
        usize: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.oracle.get(k)
    }

    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<Oracle<G::EV, G::C>>
    where
        usize: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        self.oracle.remove(k)
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, usize, Oracle<G::EV, G::C>> {
        self.oracle.iter()
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
    use rand::random;
    use serde::{Deserialize, Serialize};

    use crate::oracle::block_pair::BlockPair;

    use super::Oracle;

    #[test]
    fn add_block_pair_test() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord<f64>>, f64> =
            RTreeGraph::new_from_graph(DirectedCsrGraph::default());
        let mut oracle = Oracle::new(0);

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            0.2,
            &graph,
        );

        oracle.add_block_pair(block_pair);
    }

    #[test]
    fn get_block_pairs_test() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord<f64>>, f64> =
            RTreeGraph::new_from_graph(DirectedCsrGraph::default());
        let mut oracle = Oracle::new(0);

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            0.2,
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
        let mut oracle = Oracle::new(0);

        let block_pair = BlockPair::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            0.2,
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
