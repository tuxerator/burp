use std::fmt::{self, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Node<T> {
    children: Option<Vec<Node<T>>>,
    data: T,
}

impl<T> Node<T> {
    pub fn new(data: T, children: Option<Vec<Node<T>>>) -> Self {
        Self { children, data }
    }

    pub fn with_capacity(data: T, capacity: usize) -> Self {
        Self {
            children: Some(Vec::with_capacity(capacity)),
            data,
        }
    }

    pub fn get_children(&self) -> &Option<Vec<Node<T>>> {
        &self.children
    }

    pub fn get_children_mut(&mut self) -> &mut Option<Vec<Node<T>>> {
        &mut self.children
    }

    pub fn get_data(&self) -> &T {
        &self.data
    }

    pub fn get_data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    pub fn insert_child(&mut self, node: Self) -> &mut Self {
        let children = self.children.get_or_insert(Vec::new());
        children.push(node);

        children.last_mut().expect("This should not happen")
    }

    pub fn set_children(&mut self, children: Option<Vec<Node<T>>>) {
        self.children = children;
    }

    pub fn for_each_child(&self) {}

    pub fn print_mem_addr(&self) {
        println!("Node: {self:p}");
        if let Some(ref children) = self.children {
            println!("children:");
            for child in children {
                child.print_mem_addr();
            }
        }
    }
}
