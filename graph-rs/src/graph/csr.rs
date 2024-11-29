use std::{
    any::Any,
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    io::Read,
    marker::PhantomData,
    rc::Rc,
    sync::atomic::AtomicI8,
    usize, vec,
};

use cached::{Cached, SizedCache};
use log::{debug, info};
use num_traits::AsPrimitive;
use ordered_float::{FloatCore, OrderedFloat};
use osmpbfreader::{blocks, groups, primitive_block_from_blob, OsmPbfReader};
use petgraph::{
    data::{Build, Element, ElementIterator},
    stable_graph::{self, NodeIndex, StableGraph},
    visit::{EdgeRef, IntoNodeReferences},
    Directed, Undirected,
};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use serde::{Deserialize, Serialize};

use crate::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra, DijkstraResult, ResultNode},
    types::Direction,
};
use crate::{graph::Target, input::edgelist::EdgeList, DirectedGraph, Graph, GraphError};

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Csr<EV> {
    offsets: Vec<usize>,
    targets: Vec<Target<EV>>,
}

impl<EV> Csr<EV> {
    /// Create a `CSR` from `offsets` and `targets`.
    ///
    /// Returns a new `CSR` where `offsets[i]` contains the index of the first
    /// target node in `targets`.
    pub fn new(offsets: Vec<usize>, targets: Vec<Target<EV>>) -> Csr<EV> {
        Self { offsets, targets }
    }

    pub fn node_count(&self) -> usize {
        self.offsets.len() - 1
    }

    pub fn edge_count(&self) -> usize {
        self.targets.len()
    }

    // TODO: Use Result<usize> as return value.
    pub fn degree(&self, i: usize) -> usize {
        let from = self.offsets[i];
        let to = self.offsets[i + 1];

        to - from
    }

    // TODO: Use Result<usize> as return value.
    pub fn targets(&self, i: usize) -> &[Target<EV>] {
        let from = self.offsets[i];
        let to = self.offsets[i + 1];

        &self.targets[from..to]
    }

    pub fn add_node(&mut self) -> usize {
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        self.offsets.push(*self.offsets.last().unwrap_or(&0));
        self.offsets.len() - 2
    }

    pub fn add_edge(&mut self, a: usize, b: usize, weight: EV) -> bool {
        let mut neighbors = self.targets(a).iter();
        if neighbors.find(|n| n.target() == b).is_some() {
            return false;
        }
        self.targets.insert(self.offsets[a], Target::new(b, weight));

        self.offsets.par_iter_mut().enumerate().for_each(|(i, x)| {
            if i > a {
                *x += 1;
            }
        });

        true
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DirectedCsrGraph<EV, NV> {
    pub node_values: Vec<NV>,
    pub csr_out: Csr<EV>,
    pub csr_inc: Csr<EV>,
    dijkstra_cache: HashMap<(usize, usize), ResultNode<EV>>,
}

impl<EV: Clone, NV> DirectedCsrGraph<EV, NV> {
    pub fn new(
        node_values: Vec<NV>,
        csr_out: Csr<EV>,
        csr_inc: Csr<EV>,
    ) -> DirectedCsrGraph<EV, NV> {
        assert_eq!(
            csr_out.node_count(),
            csr_inc.node_count(),
            "csr_out and csr_in have different node counts"
        );
        let node_count = csr_out.node_count();
        let g = Self {
            node_values,
            csr_out,
            csr_inc,
            dijkstra_cache: HashMap::with_capacity(node_count.pow(2)),
        };

        info!(
            "Created directed graph (node_count: {:?}, edge_count = {:?})",
            g.node_count(),
            g.edge_count()
        );

        g
    }
}

impl<EV, NV> DirectedCsrGraph<EV, NV>
where
    EV: Send + Sync,
{
    pub fn par_out_neighbors<'a>(&'a self, node_id: usize) -> rayon::slice::Iter<'_, Target<EV>> {
        self.csr_out.targets(node_id).par_iter()
    }
}

impl<EV: PartialEq, NV: PartialEq> PartialEq for DirectedCsrGraph<EV, NV> {
    fn eq(&self, other: &Self) -> bool {
        self.node_values.eq(&other.node_values)
            && self.csr_out.eq(&other.csr_out)
            && self.csr_inc.eq(&other.csr_inc)
    }
}

impl<EV: Clone, NV> Graph<EV, NV> for DirectedCsrGraph<EV, NV> {
    fn node_count(&self) -> usize {
        self.csr_out.node_count()
    }

    fn edge_count(&self) -> usize {
        self.csr_out.edge_count()
    }

    // TODO: Use Result<usize> as return value.
    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a,
    {
        let mut dict: HashSet<&Target<EV>> = HashSet::new();
        self.out_neighbors(node)
            .chain(self.in_neighbors(node))
            .filter(move |&x| {
                if !dict.contains(x) {
                    dict.insert(x);
                    true
                } else {
                    false
                }
            })
    }

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut visited: Rc<RefCell<HashSet<usize>>> = Rc::new(RefCell::new(HashSet::new()));

        self.node_values().flat_map(move |node| {
            let node_id = node.0;
            {
                visited.borrow_mut().insert(node_id);
            }
            let visited_ref = visited.clone();
            self.neighbors(node_id).filter_map(move |neighbor| {
                if !visited_ref.borrow().contains(&neighbor.target()) {
                    Some((node_id, neighbor.target()))
                } else {
                    None
                }
            })
        })
    }

    // TODO: Use Result<usize> as return value.
    fn degree(&self, node: usize) -> usize {
        self.out_degree(node) + self.in_degree(node)
    }

    fn node_value(&self, node: usize) -> Option<&NV> {
        self.node_values.get(node)
    }

    fn node_values<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a,
    {
        self.node_values.iter().enumerate()
    }

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV> {
        self.node_values.get_mut(node)
    }

    /// Returns an Iterator over all nodes.
    ///
    /// The Iterator yields pairs `(i, val)`, where `i` is the index
    /// of the node and `val` the data accociated with that node.

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), crate::GraphError> {
        let node_value = self
            .node_values
            .get_mut(node)
            .ok_or(GraphError::EmptyNode(node))?;

        *node_value = value;

        Ok(())
    }

    fn add_node(&mut self, weight: NV) -> usize {
        let id_out = self.csr_out.add_node();
        let id_in = self.csr_inc.add_node();
        assert_eq!(id_out, id_in);

        self.node_values.push(weight);
        assert_eq!(id_out, self.node_values.len() - 1);
        id_out
    }

    fn add_edge(&mut self, a: usize, b: usize, weight: EV) -> bool {
        let ret_out = self.csr_out.add_edge(a, b, weight.clone());
        let ret_in = self.csr_inc.add_edge(b, a, weight);
        assert_eq!(ret_out, ret_in, "csr_out and csr_in are inconsitent");

        ret_out
    }
}

impl<EV: Clone, NV> DirectedGraph<EV, NV> for DirectedCsrGraph<EV, NV> {
    // TODO: Use Result<usize> as return value.
    fn out_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a,
    {
        self.csr_out.targets(node).iter()
    }

    // TODO: Use Result<usize> as return value.
    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>>
    where
        EV: 'a,
    {
        self.csr_inc.targets(node).iter()
    }

    // TODO: Use Result<usize> as return value.
    fn out_degree(&self, node: usize) -> usize {
        self.csr_out.degree(node)
    }

    // TODO: Use Result<usize> as return value.
    fn in_degree(&self, node: usize) -> usize {
        self.csr_inc.degree(node)
    }
}

impl<EV, NV> CachedDijkstra<EV, NV> for DirectedCsrGraph<EV, NV>
where
    EV: FloatCore,
{
    fn cached_dijkstra(
        &mut self,
        start_node: usize,
        target_set: HashSet<usize>,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<EV>> {
        let s_t_pairs = std::iter::repeat(start_node).zip(target_set.into_iter());
        let mut result_set = DijkstraResult(HashSet::new());
        let mut cache_misses = HashSet::new();
        for s_t in s_t_pairs {
            if let Some(result) = self.dijkstra_cache.cache_get(&s_t) {
                result_set.0.insert(result.clone());
            } else {
                cache_misses.insert(s_t.1);
            };
        }
        if !cache_misses.is_empty() {
            let result = self.dijkstra(start_node, cache_misses, direction)?;

            for t in result.0.into_iter() {
                result_set.0.insert(t.clone());
                self.dijkstra_cache.cache_set((start_node, t.node_id()), t);
            }
        }

        Some(result_set)
    }

    fn cached_dijkstra_full(
        &mut self,
        start_node: usize,
        direction: Direction,
    ) -> Option<crate::algorithms::dijkstra::DijkstraResult<EV>> {
        todo!()
    }
}

impl<EV> From<EdgeList<EV>> for DirectedCsrGraph<EV, ()>
where
    EV: Copy + Default + Send + Sync,
{
    fn from(edge_list: EdgeList<EV>) -> Self {
        let degrees_out = edge_list.degrees(Direction::Outgoing);
        let degrees_in = edge_list.degrees(Direction::Incoming);

        let mut offsets_out = prefix_sum(degrees_out);
        let mut offsets_in = prefix_sum(degrees_in);

        let edge_count_out = offsets_out.last().unwrap();
        let edge_count_in = offsets_in.last().unwrap();

        // Should be equal.
        assert_eq!(
            edge_count_in, edge_count_out,
            "edge_count_in and edge_count_out are not equal. edge_count_in: {}, edge_count_out: {}",
            edge_count_in, edge_count_out
        );

        let mut targets_out = Vec::<Target<EV>>::with_capacity(*edge_count_out);
        let mut targets_in = Vec::<Target<EV>>::with_capacity(*edge_count_in);

        targets_out.resize_with(*edge_count_out, || Target::new(0, Default::default()));
        targets_in.resize_with(*edge_count_in, || Target::new(0, Default::default()));

        edge_list.edges().for_each(|(s, t, v)| {
            let offset_out = offsets_out[s];
            let offset_in = offsets_in[t];

            // Increment offset by one after inserting target.
            offsets_out[s] = offset_out + 1;
            offsets_in[t] = offset_in + 1;

            targets_out[offset_out] = Target::new(t, v);
            targets_in[offset_in] = Target::new(s, v);
        });

        offsets_out.rotate_right(1);
        offsets_out[0] = 0;

        offsets_in.rotate_right(1);
        offsets_in[0] = 0;

        let csr_out = Csr::new(offsets_out, targets_out);
        let csr_inc = Csr::new(offsets_in, targets_in);

        DirectedCsrGraph::new(Vec::new(), csr_out, csr_inc)
    }
}

impl<NV, EV> From<petgraph::Graph<NV, EV, Directed, usize>> for DirectedCsrGraph<EV, NV>
where
    NV: Clone + Debug,
    EV: Copy + Default + Send + Sync,
{
    fn from(graph: petgraph::Graph<NV, EV, Directed, usize>) -> Self {
        let nodes = graph.node_indices();

        let degrees = nodes
            .map(|node| {
                (
                    graph.neighbors_directed(node, petgraph::Direction::Outgoing),
                    graph.neighbors_directed(node, petgraph::Direction::Incoming),
                )
            })
            .fold((vec![], vec![]), |mut acc, e| {
                acc.0.push(e.0.count());
                acc.1.push(e.1.count());
                acc
            });

        let (mut offsets_out, mut offsets_in) = (prefix_sum(degrees.0), prefix_sum(degrees.1));
        let edge_counts = (offsets_out.last().unwrap(), offsets_in.last().unwrap());

        let mut targets_out = Vec::new();
        let mut targets_in = Vec::new();

        targets_out.resize_with(*edge_counts.0, || Target::new(0, Default::default()));
        targets_in.resize_with(*edge_counts.1, || Target::new(0, Default::default()));

        graph.edge_references().for_each(|edge| {
            let offset_out = offsets_out[edge.source().index()];
            let offset_in = offsets_in[edge.target().index()];

            // Increment offset by one after inserting target.
            offsets_out[edge.source().index()] = offset_out + 1;
            offsets_in[edge.target().index()] = offset_in + 1;

            targets_out[offset_out] = Target::new(edge.target().index(), *edge.weight());
            targets_in[offset_in] = Target::new(edge.source().index(), *edge.weight());
        });

        offsets_out.rotate_right(1);
        offsets_out[0] = 0;

        offsets_in.rotate_right(1);
        offsets_in[0] = 0;

        let csr_out = Csr::new(offsets_out, targets_out);

        let csr_inc = Csr::new(offsets_in, targets_in);

        let node_values = graph
            .node_weights()
            .map(|node| node.clone())
            .collect::<Vec<_>>();

        DirectedCsrGraph::new(node_values, csr_out, csr_inc)
    }
}

fn prefix_sum(degrees: Vec<usize>) -> Vec<usize> {
    let mut last = *degrees.last().unwrap();
    let mut sums: Vec<usize> = degrees
        .into_iter()
        .scan(0, |total, degree| {
            let value = *total;
            *total += degree;
            Some(value)
        })
        .collect();

    last += *sums.last().unwrap();
    sums.push(last);

    sums
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;
    use crate::graph::Target;

    static INIT: Once = Once::new();

    #[test]
    fn csr_from_vectors() {
        let offsets = vec![0, 2, 3, 4];
        let targets = vec![
            Target::new_without_value(2),
            Target::new_without_value(3),
            Target::new_without_value(1),
            Target::new_without_value(0),
        ];
        let csr = Csr::new(offsets.clone(), targets.clone());
        assert_eq!(csr.targets(1), targets.get(offsets[1]..offsets[2]).unwrap(),);
    }

    #[test]
    fn csr_degrees() {
        let offsets = vec![0, 2, 3, 4];
        let targets = vec![
            Target::new_without_value(2),
            Target::new_without_value(3),
            Target::new_without_value(1),
            Target::new_without_value(0),
        ];
        let csr = Csr::new(offsets.clone(), targets.clone());
        assert_eq!(csr.degree(0), 2);
        assert_eq!(csr.degree(2), 1);
    }

    #[test]
    fn csr_edge_count() {
        let offsets = vec![0, 2, 3, 4];
        let targets = vec![
            Target::new_without_value(2),
            Target::new_without_value(3),
            Target::new_without_value(1),
            Target::new_without_value(0),
        ];
        let csr = Csr::new(offsets.clone(), targets.clone());

        assert_eq!(csr.edge_count(), 4, "Edgecount should be 4.");
    }

    #[test]
    fn directed_csr_neighbors() {
        let offsets = vec![0, 2, 3, 4, 4];
        let targets = vec![
            Target::new_without_value(2),
            Target::new_without_value(3),
            Target::new_without_value(1),
            Target::new_without_value(0),
        ];
        let csr_out = Csr::new(offsets.clone(), targets.clone());

        let offsets = vec![0, 2, 3, 4, 5];
        let targets = vec![
            Target::new_without_value(1),
            Target::new_without_value(3),
            Target::new_without_value(4),
            Target::new_without_value(2),
            Target::new_without_value(2),
        ];
        let csr_inc = Csr::new(offsets.clone(), targets.clone());

        let directed_csr: DirectedCsrGraph<(), _> =
            DirectedCsrGraph::new(Vec::<()>::new(), csr_out, csr_inc);

        assert_eq!(
            directed_csr
                .out_neighbors(0)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![2, 3],
            "Out neighbors of node 0."
        );
        assert_eq!(
            directed_csr
                .in_neighbors(3)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![2],
            "Inc neighbors of node 3."
        );
    }

    #[test]
    fn from_edgelist() {
        let edges = EdgeList::new(vec![
            (0, 3, 0),
            (0, 5, 0),
            (1, 0, 0),
            (1, 5, 0),
            (2, 4, 0),
            (3, 0, 0),
            (3, 2, 0),
            (4, 1, 0),
            (30, 20, 0),
        ]);

        let csr = DirectedCsrGraph::from(edges);

        assert_eq!(csr.node_count(), 31);
        assert_eq!(csr.degree(0), 4, "Total degree of node 0.");
        assert_eq!(csr.in_degree(0), 2, "In degree of node 0.");
        assert_eq!(csr.out_degree(1), 2, "Out degree of node 1.");
        assert_eq!(csr.degree(22), 0, "Total degree of node 0 should be 0.");
        assert_eq!(csr.in_degree(30), 0, "In degree of node 30 should be 0.");
        assert_eq!(
            csr.out_neighbors(3)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![0, 2],
            "Outgoing neighbors for node 3:"
        );
        assert_eq!(
            csr.in_neighbors(5)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![0, 1],
            "Incoming neighbors for node 5:"
        );
    }

    #[test]
    fn from_petgraph() {
        let petgraph: petgraph::Graph<f64, usize, _, usize> = petgraph::Graph::from_edges(vec![
            (0, 3, 0),
            (0, 5, 0),
            (1, 0, 0),
            (1, 5, 0),
            (2, 4, 0),
            (3, 0, 0),
            (3, 2, 0),
            (4, 1, 0),
            (30, 20, 0),
        ]);

        let csr = DirectedCsrGraph::from(petgraph);

        assert_eq!(csr.node_count(), 31);
        assert_eq!(csr.degree(0), 4, "Total degree of node 0.");
        assert_eq!(csr.in_degree(0), 2, "In degree of node 0.");
        assert_eq!(csr.out_degree(1), 2, "Out degree of node 1.");
        assert_eq!(csr.degree(22), 0, "Total degree of node 0 should be 0.");
        assert_eq!(csr.in_degree(30), 0, "In degree of node 30 should be 0.");
        assert_eq!(
            csr.out_neighbors(3)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![0, 2],
            "Outgoing neighbors for node 3:"
        );
        assert_eq!(
            csr.in_neighbors(5)
                .map(|x| x.target())
                .collect::<Vec<usize>>(),
            vec![0, 1],
            "Incoming neighbors for node 5:"
        );
    }
}
