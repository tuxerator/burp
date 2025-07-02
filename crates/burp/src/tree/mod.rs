use std::pin::Pin;

use node::Node;

mod iter;
pub mod node;

#[derive(Debug)]
pub struct Tree<T> {
    root: Node<T>,
}

impl<T> Tree<T> {
    pub fn new(root: Node<T>) -> Self {
        Self { root }
    }

    pub fn get_root(&self) -> &Node<T> {
        &self.root
    }

    pub fn get_root_mut(&mut self) -> &mut Node<T> {
        &mut self.root
    }

    pub fn print_mem_addr(&self) {
        self.root.print_mem_addr();
    }
}
