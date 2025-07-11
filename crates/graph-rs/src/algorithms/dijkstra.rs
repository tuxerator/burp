use std::{
    cmp::{Ordering, Reverse},
    fmt::Debug,
    hash::Hash,
};

use log::{debug, trace};
use num_traits::{Num, Zero};
use ordered_float::{FloatCore, OrderedFloat};
use priority_queue::PriorityQueue;
use rustc_hash::{FxBuildHasher, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::{
    DirectedGraph, Graph,
    graph::{Path, Target},
    types::Direction,
};

pub trait Dijkstra: Graph
where
    Self::EV: Num,
{
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: FxHashSet<usize>,
        direction: Direction,
    ) -> DijkstraResult<Self::EV>;

    fn dijkstra_full(&self, start_node: usize, direction: Direction) -> DijkstraResult<Self::EV>;
}

default impl<G> Dijkstra for G
where
    G: DirectedGraph,
    G::EV: FloatCore,
{
    fn dijkstra(
        &self,
        start_node: usize,
        mut target_set: FxHashSet<usize>,
        direction: Direction,
    ) -> DijkstraResult<Self::EV> {
        let mut frontier = PriorityQueue::with_hasher(FxBuildHasher);
        let mut result = FxHashSet::default();
        let mut visited = FxHashSet::default();
        frontier.push(
            ResultNode::new(Target::new(start_node, Self::EV::zero()), None),
            Reverse(OrderedFloat(Self::EV::zero())),
        );

        while !target_set.is_empty() && !frontier.is_empty() {
            let node = frontier.pop().expect("This is a bug").0;
            if visited.contains(&node.node_id()) {
                continue;
            }

            let neighbours: Box<dyn Iterator<Item = &Target<Self::EV>>> = match direction {
                Direction::Outgoing => Box::new(self.out_neighbors(node.node_id())),
                Direction::Incoming => Box::new(self.in_neighbors(node.node_id())),
                Direction::Undirected => Box::new(self.neighbors(node.node_id())),
            };

            neighbours.for_each(|n| {
                let path_cost = *node.cost() + *n.value();
                let new_node =
                    ResultNode::new(Target::new(n.target(), path_cost), Some(node.node_id()));
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

            target_set.take(&node.node_id()).inspect(|node| {
                trace!("found path to node {}", node);
            });
            result.insert(node);
        }

        if !target_set.is_empty() {
            debug!("could not find a path to these nodes: {:?}", target_set);
        }

        DijkstraResult::new(result)
    }

    fn dijkstra_full(&self, start_node: usize, direction: Direction) -> DijkstraResult<Self::EV> {
        self.dijkstra(
            start_node,
            FxHashSet::from_iter(0..self.node_count()),
            direction,
        )
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct DijkstraResult<T>(pub FxHashSet<ResultNode<T>>);

impl<T: Num + Clone> DijkstraResult<T> {
    pub fn new(hash_set: FxHashSet<ResultNode<T>>) -> Self {
        Self(hash_set)
    }

    pub fn path(&self, node_id: usize) -> Option<Path<T>> {
        let node = self
            .0
            .get(&ResultNode::new(Target::new(node_id, T::zero()), None))?;

        let mut path = Path::new(vec![node.target.clone()]);
        let Some(mut node_id) = node.prev_node_id() else {
            return Some(path);
        };

        while let Some(node) = self.get(node_id) {
            path.push(node.target.clone());

            if let Some(prev_node_id) = node.prev_node_id() {
                node_id = prev_node_id;
            } else {
                break;
            }
        }

        path.path.reverse();

        Some(path)
    }

    pub fn get(&self, node_id: usize) -> Option<&ResultNode<T>> {
        self.0
            .get(&ResultNode::new(Target::new(node_id, T::zero()), None))
    }

    pub fn convert_to_path(mut self, node_id: usize) -> Vec<ResultNode<T>> {
        let mut node_id = node_id;
        let mut path = vec![];
        while let Some(node) = self
            .0
            .take(&ResultNode::new(Target::new(node_id, T::zero()), None))
        {
            let prev_node_id = node.prev_node_id();
            path.push(node);

            if prev_node_id.is_none() {
                break;
            }

            node_id = prev_node_id.unwrap();
        }

        path.reverse();

        path
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ResultNode<T> {
    target: Target<T>,
    prev_node_id: Option<usize>,
}

impl<T> ResultNode<T> {
    pub fn new(target: Target<T>, prev_node_id: Option<usize>) -> Self {
        Self {
            target,
            prev_node_id,
        }
    }

    pub fn node_id(&self) -> usize {
        self.target.target()
    }

    pub fn prev_node_id(&self) -> Option<usize> {
        self.prev_node_id
    }

    pub fn cost(&self) -> &T {
        &self.target.value()
    }
}

impl<T: Default> From<usize> for ResultNode<T> {
    fn from(value: usize) -> Self {
        Self {
            target: Target::new(value, T::default()),
            prev_node_id: None,
        }
    }
}

impl<T> PartialEq for ResultNode<T> {
    fn eq(&self, other: &Self) -> bool {
        self.target.target() == other.target.target()
    }
}

impl<T> Eq for ResultNode<T> {}

impl<T: PartialOrd> PartialOrd for ResultNode<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.target.value().partial_cmp(&other.target.value())
    }
}

impl<T> Hash for ResultNode<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.target.target().hash(state);
    }
}

#[cfg(test)]
mod test {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use ordered_float::OrderedFloat;

    use crate::{algorithms::dijkstra::ResultNode, graph::Target};

    #[test]
    fn result_node_hash() {
        let mut h_1 = DefaultHasher::new();
        let mut h_2 = DefaultHasher::new();

        ResultNode::new(Target::new(34, OrderedFloat(0.0)), None).hash(&mut h_1);
        ResultNode::new(Target::new(34, OrderedFloat(4.9)), Some(45)).hash(&mut h_2);
        assert_eq!(h_1.finish(), h_2.finish());
    }
}
