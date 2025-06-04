use rstar::{ParentNode, RTreeNode, RTreeObject};

pub fn r_tree_size<T: RTreeObject>(root: &ParentNode<T>) -> usize {
    root.children()
        .iter()
        .map(|child| match child {
            RTreeNode::Parent(node) => r_tree_size(node),
            RTreeNode::Leaf(_) => 1,
        })
        .sum()
}
