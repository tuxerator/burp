use core::f64;
use std::{
    cmp::{Ordering, Reverse},
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    hash::Hash,
    mem,
    sync::{Arc, RwLock},
    usize,
};

use log::info;
use num_traits::Num;
use ordered_float::{FloatCore, OrderedFloat};
use priority_queue::PriorityQueue;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rayon::prelude::*;

use crate::{
    graph::{self, Path, Target},
    DirectedGraph,
};

pub trait Dijkstra<T, V> {
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: HashSet<usize>,
    ) -> Result<DijkstraResult<T>, String>;

    fn dijkstra_full(&self, start_node: usize) -> Result<DijkstraResult<T>, String>;

    fn double_dijkstra(
        &self,
        start_node: usize,
        end_node: usize,
        target_set: HashSet<usize>,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, String>;
}

impl<T, V, U> Dijkstra<T, V> for U
where
    T: FloatCore + Copy + Default + Debug + Send + Sync,
    U: DirectedGraph<T, V>,
{
    fn dijkstra(
        &self,
        start_node: usize,
        mut target_set: HashSet<usize>,
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

            let neighbours = self.neighbors(node.node_id());

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

            target_set.take(&node.node_id());
            result.insert(node);
        }

        Ok(DijkstraResult::new(result))
    }

    fn dijkstra_full(&self, start_node: usize) -> Result<DijkstraResult<T>, String> {
        self.dijkstra(start_node, HashSet::from_iter(0..self.node_count()))
    }

    fn double_dijkstra(
        &self,
        start_node: usize,
        end_node: usize,
        mut target_set: HashSet<usize>,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, String> {
        let mut frontier = Arc::new(RwLock::new(PriorityQueue::new()));
        let mut visited = Arc::new(RwLock::new(HashSet::new()));
        let mut distance_estimate = OrderedFloat(T::infinity());
    }
}

fn dijkstra<G, T, NV>(
    graph: Arc<RwLock<G>>,
    start_node: usize,
    mut target_set: HashSet<usize>,
    visited: Arc<RwLock<HashSet<usize>>>,
    direction: Direction,
    result: Arc<RwLock<HashSet<ResultNode<OrderedFloat<T>>>>>,
) where
    T: FloatCore + Copy + Default + Debug + Send + Sync,
    G: DirectedGraph<T, NV> + Send + Sync,
{
    let mut frontier = PriorityQueue::new();
    frontier.push(
        ResultNode::new(start_node, None, OrderedFloat(T::zero()), direction),
        Reverse(OrderedFloat(T::zero())),
    );

    while !target_set.is_empty() && !frontier.is_empty() {
        let node = frontier.pop().unwrap().0;
        let result = result.write().expect("poisoned lock");
        if let Some(visited_node) = result.take(&node) {
            if visited_node.direction() == &direction {
                continue;
            }

            let distance = *node.cost() + *visited_node.cost();
        }

        let graph = graph.read().expect("poisoned lock");
        let neighbours: Box<dyn Iterator<Item = &Target<T>> + Send + Sync> = match direction {
            Direction::Forward => Box::new(graph.out_neighbors(node.node_id())),
            Direction::Backward => Box::new(graph.in_neighbors(node.node_id())),
            Direction::None => Box::new(graph.neighbors(node.node_id())),
        };

        neighbours.for_each(|n| {
            let path_cost = *node.cost() + *n.value();
            let new_node = ResultNode::new(n.target(), Some(node.node_id()), path_cost, direction);
            if !frontier.change_priority_by(&new_node, |p| {
                if p.0 > path_cost {
                    p.0 = path_cost
                }
            }) {
                frontier.push(new_node, Reverse(path_cost));
            }
        });

        visited
            .write()
            .expect("poisoned lock")
            .insert(node.node_id());

        target_set.take(&node.node_id());
        result.write().expect("poisoned lock").insert(node);
    }
}

#[derive(Debug)]
pub struct DijkstraResult<T>(HashSet<ResultNode<OrderedFloat<T>>>);

impl<T: FloatCore + Debug> DijkstraResult<T> {
    pub fn new(hash_set: HashSet<ResultNode<OrderedFloat<T>>>) -> Self {
        Self(hash_set)
    }

    pub fn path(&self, mut node_id: usize) -> Option<Vec<&ResultNode<OrderedFloat<T>>>> {
        let mut path = vec![];
        while let Some(node) = self.0.get(&ResultNode::new(
            node_id,
            None,
            OrderedFloat(T::zero()),
            Direction::None,
        )) {
            dbg!(node);
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
        self.0.get(&ResultNode::new(
            node_id,
            None,
            OrderedFloat(T::zero()),
            Direction::None,
        ))
    }

    pub fn convert_to_path(mut self, node_id: usize) -> Option<Vec<ResultNode<OrderedFloat<T>>>> {
        let mut node_id = Some(node_id);
        let mut path = vec![];
        while let Some(node) = self.0.take(&ResultNode::new(
            node_id?,
            None,
            OrderedFloat(T::zero()),
            Direction::None,
        )) {
            node_id = node.prev_node_id();
            path.push(node);
        }

        path.reverse();

        Some(path)
    }
}

#[derive(PartialEq, Debug)]
enum Direction {
    Forward,
    Backward,
    None,
}

#[derive(Debug)]
pub struct ResultNode<T> {
    node_id: usize,
    prev_node_id: Option<usize>,
    cost: T,
    direction: Direction,
}

impl<T: Num + Ord> ResultNode<T> {
    pub fn new(node_id: usize, prev_node_id: Option<usize>, cost: T, direction: Direction) -> Self {
        Self {
            node_id,
            prev_node_id,
            cost,
            direction,
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

    pub fn direction(&self) -> &Direction {
        &self.direction
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
