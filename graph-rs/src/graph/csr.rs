use std::{
    cell::RefCell, collections::HashSet, fmt::Debug, hash::Hash, io::Read, marker::PhantomData,
    rc::Rc, sync::atomic::AtomicI8, usize, vec,
};

use log::info;
use osmpbfreader::{blocks, groups, primitive_block_from_blob, OsmPbfReader};
use petgraph::{
    data::{Build, Element, ElementIterator},
    stable_graph::{self, NodeIndex, StableGraph},
    Directed, Undirected,
};
use rayon::iter::IntoParallelRefIterator;
use serde::{Deserialize, Serialize};

use crate::types::Direction;
use crate::{graph::Target, input::edgelist::EdgeList, DirectedGraph, Graph, GraphError};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Csr<EV> {
    offsets: Box<[usize]>,
    targets: Box<[Target<EV>]>,
}

impl<EV> Csr<EV> {
    /// Create a `CSR` from `offsets` and `targets`.
    ///
    /// Returns a new `CSR` where `offsets[i]` contains the index of the first
    /// target node in `targets`.
    pub fn new(offsets: Box<[usize]>, targets: Box<[Target<EV>]>) -> Csr<EV> {
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
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct DirectedCsrGraph<EV, NV>
where
    EV: Copy,
    NV: Clone,
{
    pub node_values: Box<[NV]>,
    pub csr_out: Csr<EV>,
    pub csr_inc: Csr<EV>,
}

impl<EV, NV> DirectedCsrGraph<EV, NV>
where
    EV: Copy + Send + Sync,
    NV: Clone,
{
    pub fn new(
        node_values: Box<[NV]>,
        csr_out: Csr<EV>,
        csr_inc: Csr<EV>,
    ) -> DirectedCsrGraph<EV, NV> {
        let g = Self {
            node_values,
            csr_out,
            csr_inc,
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
    EV: Copy + Send + Sync,
    NV: Clone,
{
    pub fn par_out_neighbors<'a>(&'a self, node_id: usize) -> rayon::slice::Iter<'_, Target<EV>> {
        self.csr_out.targets(node_id).par_iter()
    }
}

impl<EV, NV> Graph<EV, NV> for DirectedCsrGraph<EV, NV>
where
    EV: Copy + Send + Sync,
    NV: Clone,
{
    fn node_count(&self) -> usize {
        self.csr_out.node_count()
    }

    fn edge_count(&self) -> usize {
        self.csr_out.edge_count()
    }

    // TODO: Use Result<usize> as return value.
    fn neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>> + Send + Sync
    where
        EV: 'a,
    {
        let mut dict: HashSet<Target<EV>> = HashSet::new();
        self.out_neighbors(node)
            .chain(self.in_neighbors(node))
            .filter(move |&x| {
                if !dict.contains(x) {
                    dict.insert(*x);
                    true
                } else {
                    false
                }
            })
    }

    fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut visited: Rc<RefCell<HashSet<usize>>> = Rc::new(RefCell::new(HashSet::new()));

        self.iter().flat_map(move |node| {
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

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV> {
        self.node_values.get_mut(node)
    }

    /// Returns an Iterator over all nodes.
    ///
    /// The Iterator yields pairs `(i, val)`, where `i` is the index
    /// of the node and `val` the data accociated with that node.
    fn iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a,
    {
        self.node_values.iter().enumerate()
    }

    fn set_node_value(&mut self, node: usize, value: NV) -> Result<(), crate::GraphError> {
        let node_value = self
            .node_values
            .get_mut(node)
            .ok_or(GraphError::EmptyNode(node))?;

        *node_value = value;

        Ok(())
    }

    fn to_stable_graph(&self) -> StableGraph<Option<NV>, EV, Directed, usize> {
        let mut stable_graph: StableGraph<Option<NV>, EV, Directed, usize> =
            StableGraph::with_capacity(self.node_count(), self.edge_count());

        for node in 0..self.node_count() {
            let idx = stable_graph.add_node(self.node_value(node).cloned());
            let node: NodeIndex<usize> = NodeIndex::new(node);
            assert_eq!(node, idx);
        }

        for node in 0..self.node_count() {
            self.neighbors(node).for_each(|target| {
                stable_graph.add_edge(
                    NodeIndex::new(node),
                    NodeIndex::new(target.target()),
                    *target.value(),
                );
            });
        }

        stable_graph
    }
}

impl<EV, NV> DirectedGraph<EV, NV> for DirectedCsrGraph<EV, NV>
where
    EV: Copy + Send + Sync,
    NV: Clone,
{
    // TODO: Use Result<usize> as return value.
    fn out_neighbors<'a>(
        &'a self,
        node: usize,
    ) -> impl Iterator<Item = &'a Target<EV>> + Send + Sync
    where
        EV: 'a + Send + Sync,
    {
        self.csr_out.targets(node).iter()
    }

    // TODO: Use Result<usize> as return value.
    fn in_neighbors<'a>(&'a self, node: usize) -> impl Iterator<Item = &'a Target<EV>> + Send + Sync
    where
        EV: 'a + Send + Sync,
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

impl<EV> From<EdgeList<EV>> for DirectedCsrGraph<EV, ()>
where
    EV: Copy + Default,
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

        let csr_out = Csr::new(
            offsets_out.into_boxed_slice(),
            targets_out.into_boxed_slice(),
        );
        let csr_inc = Csr::new(offsets_in.into_boxed_slice(), targets_in.into_boxed_slice());

        DirectedCsrGraph {
            node_values: Box::new([()]),
            csr_out,
            csr_inc,
        }
    }
}

// impl<EV, R: Read> From<OsmPbfReader<R>> for DirectedCsrGraph<EV, osmpbfreader::NodeId>
// where
//     EV: Eq + PartialEq + Hash + Copy + Default,
// {
//     fn from(mut pbf: OsmPbfReader<R>) -> Self {
//         let mut node_ids = vec![];
//         let mut nodes = vec![];
//         let mut ways = vec![];
//         for block in pbf.blobs().map(|b| primitive_block_from_blob(&b.unwrap())) {
//             let block = block.unwrap();
//             for group in block.get_primitivegroup().iter() {
//                 for node in groups::nodes(&group, &block) {
//                     node_ids.push(node.id);
//                     nodes.push(node);
//                 }
//
//                 for way in groups::ways(&group, &block) {
//                     ways.push(way);
//                 }
//             }
//         }
//
//         node_ids.sort();
//         node_ids.dedup();
//
//         let mut edges = vec![];
//
//         for way in ways.iter() {
//             let mut node_iter = way.nodes.iter();
//
//             let mut start = node_iter.next().unwrap();
//
//             for node in node_iter {
//                 edges.push((
//                     usize::try_from(start.0).unwrap(),
//                     usize::try_from(node.0).unwrap(),
//                     EV::default(),
//                 ));
//
//                 start = node;
//             }
//         }
//
//         let edge_list = EdgeList::new(edges);
//
//         let g = DirectedCsrGraph::from(edge_list);
//
//         Self {
//             node_values: node_ids.into_boxed_slice(),
//             csr_out: g.csr_out,
//             csr_inc: g.csr_inc,
//         }
//     }
// }

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
        let csr = Csr::new(
            offsets.clone().into_boxed_slice(),
            targets.clone().into_boxed_slice(),
        );
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
        let csr = Csr::new(
            offsets.clone().into_boxed_slice(),
            targets.clone().into_boxed_slice(),
        );
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
        let csr = Csr::new(
            offsets.clone().into_boxed_slice(),
            targets.clone().into_boxed_slice(),
        );

        assert_eq!(csr.edge_count(), 4, "Edgecount should be 4.");
    }

    #[test]
    fn directed_csr_neighbors() {
        let offsets = vec![0, 2, 3, 4];
        let targets = vec![
            Target::new_without_value(2),
            Target::new_without_value(3),
            Target::new_without_value(1),
            Target::new_without_value(0),
        ];
        let csr_out = Csr::new(
            offsets.clone().into_boxed_slice(),
            targets.clone().into_boxed_slice(),
        );

        let offsets = vec![0, 2, 3, 4, 5];
        let targets = vec![
            Target::new_without_value(1),
            Target::new_without_value(3),
            Target::new_without_value(4),
            Target::new_without_value(2),
            Target::new_without_value(2),
        ];
        let csr_inc = Csr::new(
            offsets.clone().into_boxed_slice(),
            targets.clone().into_boxed_slice(),
        );

        let directed_csr = DirectedCsrGraph::new(Box::new([()]), csr_out, csr_inc);

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
