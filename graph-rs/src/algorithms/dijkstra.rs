use std::{
    cmp::{Ordering, Reverse},
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    hash::Hash,
    mem, usize,
};

use log::{debug, info};
use num_traits::Num;
use ordered_float::{FloatCore, OrderedFloat};
use priority_queue::PriorityQueue;
use rayon::iter::ParallelIterator;

use crate::{
    graph::{Path, Target},
    types::Direction,
    DirectedGraph,
};

pub trait Dijkstra<T: FloatCore, V> {
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: HashSet<usize>,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, String>;

    fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, String>;
}

impl<T, V, U> Dijkstra<T, V> for U
where
    T: FloatCore + Send + Sync + Debug,
    U: DirectedGraph<T, V>,
{
    fn dijkstra(
        &self,
        start_node: usize,
        mut target_set: HashSet<usize>,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, String> {
        let mut frontier = PriorityQueue::new();
        let mut result = HashSet::new();
        let mut visited = HashSet::new();
        frontier.push(
            ResultNode::new(start_node, None, OrderedFloat(T::zero())),
            Reverse(OrderedFloat(T::zero())),
        );

        while !target_set.is_empty() && !frontier.is_empty() {
            let node = frontier.pop().ok_or("frontier is empty".to_string())?.0;
            if visited.contains(&node.node_id()) {
                continue;
            }

            let neighbours: Box<dyn Iterator<Item = &Target<T>>> = match direction {
                Direction::Outgoing => Box::new(self.out_neighbors(node.node_id())),
                Direction::Incoming => Box::new(self.in_neighbors(node.node_id())),
                Direction::Undirected => Box::new(self.neighbors(node.node_id())),
            };

            neighbours.for_each(|n| {
                let path_cost = *node.cost() + *n.value();
                let new_node = ResultNode::new(n.target(), Some(node.node_id()), path_cost);
                if !frontier.change_priority_by(&new_node, |p| {
                    if p.0 > path_cost {
                        p.0 = path_cost
                    }
                }) {
                    frontier.push(new_node, Reverse(path_cost));
                }
            });

            visited.insert(node.node_id());

            target_set.take(&node.node_id()).and_then(|node| {
                debug!("found path to node {}", node);
                Some(node)
            });
            result.insert(node);
        }

        Ok(DijkstraResult::new(result))
    }

    fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, String> {
        self.dijkstra(
            start_node,
            HashSet::from_iter(0..self.node_count()),
            direction,
        )
    }
}

#[derive(PartialEq, Debug)]
pub struct DijkstraResult<T: FloatCore>(pub HashSet<ResultNode<OrderedFloat<T>>>);

impl<T: FloatCore> DijkstraResult<T> {
    pub fn new(hash_set: HashSet<ResultNode<OrderedFloat<T>>>) -> Self {
        Self(hash_set)
    }

    pub fn path(&self, mut node_id: usize) -> Option<Vec<&ResultNode<OrderedFloat<T>>>> {
        let mut path = vec![];
        while let Some(node) = self
            .0
            .get(&ResultNode::new(node_id, None, OrderedFloat(T::zero())))
        {
            path.push(node);

            if let Some(prev_node_id) = node.prev_node_id() {
                node_id = prev_node_id;
            } else {
                break;
            }
        }

        path.reverse();

        Some(path)
    }

    pub fn get(&self, node_id: usize) -> Option<&ResultNode<OrderedFloat<T>>> {
        self.0
            .get(&ResultNode::new(node_id, None, OrderedFloat(T::zero())))
    }

    pub fn convert_to_path(mut self, node_id: usize) -> Option<Vec<ResultNode<OrderedFloat<T>>>> {
        let mut node_id = Some(node_id);
        let mut path = vec![];
        while let Some(node) =
            self.0
                .take(&ResultNode::new(node_id?, None, OrderedFloat(T::zero())))
        {
            node_id = node.prev_node_id();
            path.push(node);
        }

        path.reverse();

        Some(path)
    }
}

#[derive(Debug)]
pub struct ResultNode<T> {
    node_id: usize,
    prev_node_id: Option<usize>,
    cost: T,
}

impl<T: Num + Ord> ResultNode<T> {
    pub fn new(node_id: usize, prev_node_id: Option<usize>, cost: T) -> Self {
        Self {
            node_id,
            prev_node_id,
            cost,
        }
    }

    pub fn node_id(&self) -> usize {
        self.node_id
    }

    pub fn prev_node_id(&self) -> Option<usize> {
        self.prev_node_id
    }

    pub fn cost(&self) -> &T {
        &self.cost
    }
}

impl<T: Num + Ord> PartialEq for ResultNode<T> {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl<T: Num + Ord> PartialOrd for ResultNode<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Num + Ord> Eq for ResultNode<T> {}

impl<T: Num + Ord> Ord for ResultNode<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost.cmp(&other.cost)
    }
}

impl<T> Hash for ResultNode<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

#[cfg(test)]
mod test {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use num_traits::Zero;
    use ordered_float::OrderedFloat;

    use crate::algorithms::dijkstra::ResultNode;

    #[test]
    fn result_node_hash() {
        let mut h_1 = DefaultHasher::new();
        let mut h_2 = DefaultHasher::new();

        ResultNode::new(34, None, OrderedFloat(0.0)).hash(&mut h_1);
        ResultNode::new(34, Some(45), OrderedFloat(4.9)).hash(&mut h_2);
        assert_eq!(h_1.finish(), h_2.finish());
    }
}
