use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Debug,
    hash::Hash,
    usize,
};

use num_traits::Num;
use priority_queue::PriorityQueue;
use rayon::iter::ParallelIterator;

use crate::{graph::Path, DirectedGraph};

pub trait Dijkstra<T: Num + Ord, V> {
    fn dijkstra(
        &self,
        start_node: usize,
        target_set: HashSet<usize>,
    ) -> Result<HashMap<usize, Path<T>>, &str>;

    fn dijkstra_full(&self, start_node: usize) -> Result<HashMap<usize, Path<T>>, &str>;
}

impl<T, V, U> Dijkstra<T, V> for U
where
    T: Num + Ord + Copy + Default + Hash,
    U: DirectedGraph<T, V>,
{
    fn dijkstra(
        &self,
        start_node: usize,
        mut target_set: HashSet<usize>,
    ) -> Result<HashMap<usize, Path<T>>, &str> {
        let mut frontier = PriorityQueue::new();
        let mut result = HashMap::new();
        frontier.push(Path::new(start_node, Vec::default()), Reverse(T::zero()));

        while !target_set.is_empty() {
            let mut node = frontier.pop().ok_or("frontier is empty")?.0;

            if target_set
                .take(&node.last_node().ok_or("path is empty")?)
                .is_some()
            {
                result.insert(node.last_node().ok_or("path is empty")?, node.clone());
            }

            let neighbours = self.neighbors(node.last_node().ok_or("path is empty")?);

            neighbours.for_each(|n| {
                let mut path = node.clone();
                path.push(*n);
                let path_cost = path.cost();
                if !frontier.change_priority_by(&path, |p| {
                    if p.0 > path_cost {
                        p.0 = path_cost
                    }
                }) {
                    frontier.push(path.clone(), Reverse(path_cost));
                }
            })
        }

        Ok(result)
    }

    fn dijkstra_full(&self, start_node: usize) -> Result<HashMap<usize, Path<T>>, &str> {
        self.dijkstra(start_node, HashSet::from_iter(0..self.node_count()))
    }
}
