use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
    fmt::Debug,
    usize,
};

use num_traits::Num;
use priority_queue::PriorityQueue;
use rayon::iter::ParallelIterator;

use crate::DirectedGraph;

pub trait Dijkstra<T: Num + Ord, V> {
    fn dijkstra(&self, start_node: usize, target_set: HashSet<usize>) -> HashMap<usize, T>;
}

impl<T, V, U> Dijkstra<T, V> for U
where
    T: Num + Ord + Copy,
    U: DirectedGraph<T, V>,
{
    fn dijkstra(&self, start_node: usize, mut target_set: HashSet<usize>) -> HashMap<usize, T> {
        let mut frontier = PriorityQueue::new();
        let mut result = HashMap::new();
        frontier.push(start_node, Reverse(T::zero()));

        while !target_set.is_empty() {
            let mut node = frontier.pop().ok_or("frontier is empty").unwrap();

            if let Some(_) = target_set.take(&node.0) {
                result.insert(node.0, node.1 .0);
            }

            let neighbours = self.neighbors(node.0);

            neighbours.for_each(|n| {
                if !frontier.change_priority_by(&n.target(), |p| {
                    if p > &mut node.1 {
                        p = &mut node.1
                    }
                }) {
                    frontier.push(n.target(), Reverse(node.1 .0 + *n.value()));
                }
            })
        }

        result
    }
}
