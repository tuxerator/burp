use std::{
    cmp::{Ordering, Reverse},
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    hash::Hash,
    mem,
    sync::{Arc, PoisonError, RwLock},
    thread, usize,
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
        direction: Direction,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>>;

    fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>>;

    fn double_dijkstra(
        &self,
        start_node: usize,
        end_node: usize,
        target_set: HashSet<usize>,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>>;
}

impl<T, V, U> Dijkstra<T, V> for U
where
    T: FloatCore + Copy + Default + Debug + Send + Sync + 'static,
    U: DirectedGraph<T, V> + Send + Sync,
{
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: HashSet<usize>,
        direction: Direction,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>> {
        let mut result = Arc::new(RwLock::new(HashSet::new()));
        dijkstra(
            self,
            start_node,
            target_set,
            direction,
            result.clone(),
            epsilon,
        );

        Ok(DijkstraResult::new(
            Arc::into_inner(result)
                .ok_or("More than one strong reference")?
                .into_inner()?,
        ))
    }

    fn dijkstra_full(
        &self,
        start_node: usize,
        direction: Direction,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>> {
        self.dijkstra(
            start_node,
            HashSet::from_iter(0..self.node_count()),
            direction,
            T::infinity(),
        )
    }

    fn double_dijkstra(
        &self,
        start_node: usize,
        end_node: usize,
        target_set: HashSet<usize>,
        epsilon: T,
    ) -> Result<DijkstraResult<T>, Box<dyn Error>> {
        let result = Arc::new(RwLock::new(HashSet::default()));

        thread::scope(|s| {
            let forward_scan = s.spawn(|| {
                dijkstra(
                    self,
                    start_node,
                    target_set.clone(),
                    Direction::Forward,
                    result.clone(),
                    epsilon,
                )
            });
            let backward_scan = s.spawn(|| {
                dijkstra(
                    self,
                    end_node,
                    target_set.clone(),
                    Direction::Backward,
                    result.clone(),
                    epsilon,
                )
            });
            forward_scan.join();
            backward_scan.join();
        });

        Ok(DijkstraResult(
            Arc::into_inner(result)
                .ok_or("More than one strong reference")?
                .into_inner()?,
        ))
    }
}

fn dijkstra<G, T, NV>(
    graph: &G,
    start_node: usize,
    mut target_set: HashSet<usize>,
    direction: Direction,
    result: Arc<RwLock<HashSet<ResultNode<OrderedFloat<T>>>>>,
    epsilon: T,
) where
    T: FloatCore + Copy + Default + Debug + Send + Sync,
    G: DirectedGraph<T, NV> + Send + Sync,
{
    let mut frontier = PriorityQueue::new();
    let mut distance_bound = T::infinity();
    frontier.push(
        ResultNode::new(start_node, None, OrderedFloat(T::zero()), direction),
        Reverse(OrderedFloat(T::zero())),
    );

    while let Some((node, priority)) = frontier.pop() {
        if target_set.is_empty() || **node.cost() > distance_bound {
            break;
        }
        {
            let result = result.read().expect("poisoned lock");
            if let Some(_) = result.get(&node) {
                continue;
            }

            if let Some(visited_node) = result.get(&ResultNode::new(
                node.node_id(),
                node.prev_node_id(),
                *node.cost(),
                node.direction().inverse(),
            )) {
                distance_bound =
                    (**node.cost() + **visited_node.cost()) * (T::from(1.0).unwrap() + epsilon);
            }

            let neighbours: Box<dyn Iterator<Item = &Target<T>> + Send + Sync> = match &direction {
                Direction::Forward => Box::new(graph.out_neighbors(node.node_id())),
                Direction::Backward => Box::new(graph.in_neighbors(node.node_id())),
                Direction::None => Box::new(graph.neighbors(node.node_id())),
            };

            neighbours.for_each(|n| {
                let path_cost = *node.cost() + *n.value();
                let new_node =
                    ResultNode::new(n.target(), Some(node.node_id()), path_cost, direction);
                if !frontier.change_priority_by(&new_node, |p| {
                    if p.0 > path_cost {
                        p.0 = path_cost
                    }
                }) {
                    frontier.push(new_node, Reverse(path_cost));
                }
            });
        }

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
        while let Some(node) = self.get(node_id, Direction::None) {
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

    pub fn in_path(&self, mut node_id: usize) -> Option<Vec<&ResultNode<OrderedFloat<T>>>> {
        let mut path = vec![];
        let in_path_node = [
            ResultNode::new(node_id, None, OrderedFloat(T::zero()), Direction::Forward),
            ResultNode::new(node_id, None, OrderedFloat(T::zero()), Direction::Backward),
        ];
        if HashSet::from(in_path_node).is_subset(&self.0) {
            let mut node_fwd = node_id;
            let mut node_bwd = node_id;
            while let Some(node) = self.get(node_fwd, Direction::Forward) {
                dbg!(node);
                path.push(node);

                if let Some(prev_node_id) = node.prev_node_id() {
                    node_fwd = prev_node_id;
                } else {
                    break;
                }
            }

            path.reverse();

            while let Some(node) = self.get(node_bwd, Direction::Backward) {
                dbg!(node);
                path.push(node);

                if let Some(prev_node_id) = node.prev_node_id() {
                    node_bwd = prev_node_id;
                } else {
                    break;
                }
            }
            Some(path)
        } else {
            None
        }
    }

    pub fn get(
        &self,
        node_id: usize,
        direction: Direction,
    ) -> Option<&ResultNode<OrderedFloat<T>>> {
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

#[derive(PartialEq, Hash, Debug, Clone, Copy)]
pub enum Direction {
    Forward,
    Backward,
    None,
}

impl Direction {
    pub fn inverse(&self) -> Direction {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
            Self::None => Self::None,
        }
    }
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

    pub fn new_hashable(node_id: usize, direction: Direction) -> Self {
        Self {
            node_id,
            prev_node_id: None,
            cost: T::zero(),
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
        self.direction.hash(state);
    }
}

#[cfg(test)]
mod test {
    use std::hash::{DefaultHasher, Hash, Hasher};

    use ordered_float::OrderedFloat;

    use crate::algorithms::dijkstra::{Direction, ResultNode};

    #[test]
    fn result_node_hash() {
        let mut h_1 = DefaultHasher::new();
        let mut h_2 = DefaultHasher::new();

        ResultNode::new(34, None, OrderedFloat(0.0), Direction::Forward).hash(&mut h_1);
        ResultNode::new(34, Some(45), OrderedFloat(4.9), Direction::Forward).hash(&mut h_2);
        assert_eq!(h_1.finish(), h_2.finish());
    }
}
