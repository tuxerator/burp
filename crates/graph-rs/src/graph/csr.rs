use std::{
    cell::RefCell,
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fmt::Debug,
    rc::Rc,
    vec,
};

use log::{debug, info, trace};
use ordered_float::{FloatCore, OrderedFloat};
use priority_queue::PriorityQueue;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::{DirectedGraph, Graph, GraphError, graph::Target, input::edgelist::EdgeList};
use crate::{
    algorithms::dijkstra::{Dijkstra, DijkstraResult, ResultNode},
    types::Direction,
};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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
        if neighbors.any(|n| n.target() == b) {
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

    pub fn remove_node(&mut self, node: usize) -> bool {
        let mut edges = Vec::new();
        self.offsets.windows(2).enumerate().for_each(|n| {
            if self.targets[n.1[0]..n.1[1]]
                .iter()
                .any(|t| t.target() == node)
            {
                edges.push((n.0, node));
            }
        });

        for ele in edges {
            self.remove_edge(ele);
        }
        let targets_len = self.targets.len();
        let offsets_len = self.offsets.len();
        if offsets_len <= node + 1 {
            return false;
        }

        let offset_lower = self.offsets[node];
        let offset_upper = self.offsets[node + 1];
        let n_out_edges = offset_upper - offset_lower;

        if targets_len <= offset_upper {
            return false;
        }

        self.targets.drain(offset_lower..offset_upper);

        let targets_len = self.targets.len();

        self.targets[offset_lower..targets_len]
            .iter_mut()
            .for_each(|target| {
                if target.target > node {
                    target.target -= 1;
                }
            });
        self.offsets.remove(node);
        let offset_len = self.offsets.len();
        self.offsets[node..offset_len]
            .par_iter_mut()
            .for_each(|x| *x -= n_out_edges);

        true
    }

    pub fn remove_edge(&mut self, edge: (usize, usize)) -> Option<EV> {
        if self.offsets[edge.0] < self.offsets[edge.0 + 1] {
            let mut i = self.offsets[edge.0];
            let target_index = loop {
                if i >= self.offsets[edge.0 + 1] {
                    break None;
                }

                if self.targets[i].target() == edge.1 {
                    break Some(i);
                }

                i += 1;
            }?;

            let offset_len = self.offsets.len();
            self.offsets[edge.0 + 1..offset_len]
                .par_iter_mut()
                .for_each(|x| *x -= 1);
            Some(self.targets.remove(target_index).value)
        } else {
            None
        }
    }
}

impl<EV> Default for Csr<EV> {
    fn default() -> Self {
        Csr {
            offsets: vec![0],
            targets: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DirectedCsrGraph<EV, NV> {
    pub node_values: Vec<NV>,
    pub csr_out: Csr<EV>,
    pub csr_inc: Csr<EV>,
    #[serde(skip)]
    dijkstra_cache: RefCell<FxHashMap<usize, DijkstraResult<EV>>>,
}

impl<EV, NV> DirectedCsrGraph<EV, NV>
where
    EV: Clone + Default,
{
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
            dijkstra_cache: RefCell::new(FxHashMap::with_capacity_and_hasher(
                node_count.pow(2),
                FxBuildHasher,
            )),
        };

        info!(
            "Created directed graph (node_count: {:?}, edge_count = {:?})",
            g.node_count(),
            g.edge_count()
        );

        g
    }

    pub fn filter<F>(self, predicate: F) -> DirectedCsrGraph<EV, NV>
    where
        F: Fn(&(usize, &NV)) -> bool + Clone,
        NV: Clone,
    {
        let mut index: usize = 0;
        let mut node_map = HashMap::new();
        let mut new_graph = DirectedCsrGraph::default();

        self.nodes_iter()
            .filter(predicate.clone())
            .for_each(|node| {
                node_map.insert(node.0, index);
                new_graph.add_node(node.1.clone());

                index += 1;
            });

        node_map.iter().for_each(|node| {
            let neighbors = self.out_neighbors(*node.0).filter(|target| {
                let Some(node_value) = self.node_value(target.target()) else {
                    return false;
                };
                predicate(&(target.target(), node_value))
            });

            neighbors.for_each(|t| {
                new_graph.add_edge(
                    *node.1,
                    *node_map
                        .get(&t.target())
                        .expect("target node was not in the map"),
                    t.value().clone(),
                );
            });
        });

        new_graph
    }
    pub fn par_out_neighbors(&self, node_id: usize) -> rayon::slice::Iter<'_, Target<EV>>
    where
        EV: Send + Sync,
    {
        self.csr_out.targets(node_id).par_iter()
    }
}

impl<EV, NV> Default for DirectedCsrGraph<EV, NV>
where
    EV: Clone + Default,
{
    fn default() -> Self {
        DirectedCsrGraph::new(vec![], Csr::default(), Csr::default())
    }
}

impl<EV: PartialEq, NV: PartialEq> PartialEq for DirectedCsrGraph<EV, NV> {
    fn eq(&self, other: &Self) -> bool {
        self.node_values.eq(&other.node_values)
            && self.csr_out.eq(&other.csr_out)
            && self.csr_inc.eq(&other.csr_inc)
    }
}

impl<EV: Clone + Default, NV> Graph for DirectedCsrGraph<EV, NV> {
    type EV = EV;
    type NV = NV;
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
        let visited: Rc<RefCell<FxHashSet<usize>>> = Rc::new(RefCell::new(FxHashSet::default()));

        self.nodes_iter().flat_map(move |node| {
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

    /// Returns an Iterator over all nodes.
    ///
    /// The Iterator yields pairs `(i, val)`, where `i` is the index
    /// of the node and `val` the data accociated with that node.
    fn nodes_iter<'a>(&'a self) -> impl Iterator<Item = (usize, &'a NV)>
    where
        NV: 'a,
    {
        self.node_values.iter().enumerate()
    }

    fn node_value_mut(&mut self, node: usize) -> Option<&mut NV> {
        self.node_values.get_mut(node)
    }

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

    fn remove_node(&mut self, node: usize) -> Option<NV> {
        self.csr_inc.remove_node(node);
        self.csr_out.remove_node(node);

        if self.node_values.len() <= node {
            return None;
        }

        dbg!(self.node_values.len());

        Some(self.node_values.remove(node))
    }

    fn remove_edge(&mut self, edge: (usize, usize)) -> Option<EV> {
        self.csr_inc.remove_edge((edge.1, edge.0));
        self.csr_out.remove_edge(edge)
    }
}

impl<EV: Clone + Default, NV> DirectedGraph for DirectedCsrGraph<EV, NV> {
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

impl<EV, NV> Dijkstra for DirectedCsrGraph<EV, NV>
where
    EV: FloatCore + Default + Debug + Clone,
{
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: FxHashSet<usize>,
        direction: Direction,
    ) -> DijkstraResult<EV> {
        let mut cache = self.dijkstra_cache.borrow_mut();

        let entry = cache.entry(start_node);

        let result = entry.or_insert(DijkstraResult(FxHashSet::default()));

        // Get nodes which are not in the cached result
        let target_set: FxHashSet<ResultNode<EV>> =
            FxHashSet::from_iter(target_set.iter().map(|e| (*e).into()));
        let mut target_set = FxHashSet::from_iter(target_set.difference(&result.0).cloned());

        let mut frontier = PriorityQueue::with_hasher(FxBuildHasher);
        let mut visited = FxHashSet::default();
        frontier.push(
            ResultNode::new(start_node, None, Self::EV::zero()),
            Reverse(OrderedFloat(EV::zero())),
        );

        debug!("Computing Dijkstra for {} nodes", target_set.len());

        while !target_set.is_empty() && !frontier.is_empty() {
            let node = frontier.pop().expect("This is a bug").0;
            if visited.contains(&node.node_id()) {
                continue;
            }

            let neighbours: Box<dyn Iterator<Item = &Target<EV>>> = match direction {
                Direction::Outgoing => Box::new(self.out_neighbors(node.node_id())),
                Direction::Incoming => Box::new(self.in_neighbors(node.node_id())),
                Direction::Undirected => Box::new(self.neighbors(node.node_id())),
            };

            neighbours.for_each(|n| {
                let path_cost = *node.cost() + *n.value();
                let new_node = ResultNode::new(n.target(), Some(node.node_id()), path_cost);
                let path_cost = Reverse(OrderedFloat(path_cost));
                if let Some(priority) = frontier.get_priority(&new_node) {
                    if priority < &path_cost {
                        frontier.change_priority(&new_node, path_cost);
                    }
                } else {
                    frontier.push(new_node, path_cost);
                }
                // if !frontier.change_priority_by(&new_node, |p| {
                //     if p.0 > path_cost {
                //         p.0 = path_cost
                //     }
                // }) {
                //     frontier.push(new_node, Reverse(path_cost));
                // }
            });

            visited.insert(node.node_id());

            target_set.take(&node).inspect(|node| {
                trace!("found path to node {}", node.node_id());
            });
            result.0.insert(node);
        }

        if !target_set.is_empty() {
            debug!("could not find a path to these nodes: {:?}", target_set);
        }

        result.clone()
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
            "edge_count_in and edge_count_out are not equal. edge_count_in: {edge_count_in}, edge_count_out: {edge_count_out}"
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
        let mut node_values = Vec::new();
        node_values.resize(csr_out.node_count(), ());

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
    use super::*;
    use crate::graph::Target;

    fn setup() -> DirectedCsrGraph<i32, ()> {
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

        DirectedCsrGraph::from(edges)
    }

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
    fn remove_edge() {
        let mut graph = setup();
        let edges = EdgeList::new(vec![
            (0, 3, 0),
            (1, 0, 0),
            (1, 5, 0),
            (2, 4, 0),
            (3, 0, 0),
            (3, 2, 0),
            (4, 1, 0),
            (30, 20, 0),
        ]);

        let expected = DirectedCsrGraph::from(edges);

        assert_eq!(graph.remove_edge((0, 5)), Some(0));

        assert_eq!(graph, expected);
    }

    #[test]
    fn remove_node() {
        let mut graph = setup();
        let edges = EdgeList::new(vec![
            (0, 3, 0),
            (1, 0, 0),
            (2, 4, 0),
            (3, 0, 0),
            (3, 2, 0),
            (4, 1, 0),
            (29, 19, 0),
        ]);

        let expected = DirectedCsrGraph::from(edges);

        graph.remove_node(5);

        assert_eq!(graph, expected);
    }

    #[test]
    fn filter() {
        let graph = setup();
        let edges = EdgeList::new(vec![
            (0, 3, 0),
            (1, 0, 0),
            (2, 4, 0),
            (3, 2, 0),
            (3, 0, 0),
            (4, 1, 0),
            (29, 19, 0),
        ]);

        let expected = DirectedCsrGraph::from(edges);

        let graph = graph.filter(|n| n.0 != 5);

        assert_eq!(graph, expected);
    }
}
