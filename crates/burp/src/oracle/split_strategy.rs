use std::fmt::{Debug, Display};

use graph_rs::{CoordGraph, algorithms::dijkstra::Dijkstra};

use crate::oracle::{block_pair::BlockPair, oracle::Radius};

/// Defines how to split a [BlockPair] into children.
pub trait SplitStrategy: Display {
    fn split<G>(block_pair: &BlockPair<G::EV, G::C>, graph: &G) -> Vec<BlockPair<G::EV, G::C>>
    where
        G: CoordGraph + Dijkstra + Radius,
        G::EV: ordered_float::FloatCore + Debug,
        G::C: rstar::RTreeNum + geo::CoordFloat;
}

/// Split both blocks into 4 children.
#[derive(Debug, Clone, Copy)]
pub struct SimpleSplitStrategy;

impl Display for SimpleSplitStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Simple Split Stragety")
    }
}

impl SplitStrategy for SimpleSplitStrategy {
    fn split<G>(block_pair: &BlockPair<G::EV, G::C>, graph: &G) -> Vec<BlockPair<G::EV, G::C>>
    where
        G: CoordGraph + Dijkstra + Radius,
        G::EV: ordered_float::FloatCore + Debug,
        G::C: rstar::RTreeNum + geo::CoordFloat,
    {
        let children = (
            block_pair
                .s_block()
                .split_y()
                .into_iter()
                .flat_map(|split| split.split_x()),
            block_pair
                .t_block()
                .split_y()
                .into_iter()
                .flat_map(|split| split.split_x()),
        );

        let children = (
            children
                .0
                .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
            children
                .1
                .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
        );
        children
            .0
            .into_iter()
            .flat_map(|s_block| {
                children.1.iter().map(move |t_block| {
                    BlockPair::new(
                        s_block,
                        *t_block,
                        block_pair.poi_id(),
                        block_pair.values().epsilon,
                        graph,
                    )
                })
            })
            .collect()
    }
}

/// Split only one block on its long edge.
#[derive(Debug, Clone, Copy)]
pub struct MinimalSplitStrategy;

impl Display for MinimalSplitStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mininmal Split Stragety")
    }
}

impl SplitStrategy for MinimalSplitStrategy {
    fn split<G>(block_pair: &BlockPair<G::EV, G::C>, graph: &G) -> Vec<BlockPair<G::EV, G::C>>
    where
        G: CoordGraph + Dijkstra + Radius,
        G::EV: ordered_float::FloatCore + Debug,
        G::C: rstar::RTreeNum + geo::CoordFloat,
    {
        let r_a = block_pair.values().r_af.cost() + block_pair.values().r_ab.cost();
        let r_b = block_pair.values().r_bf.cost() + block_pair.values().r_bb.cost();

        let children = if r_a < r_b {
            if block_pair.t_block().width() < block_pair.t_block().height() {
                (
                    vec![*block_pair.s_block()],
                    block_pair.t_block().split_y().to_vec(),
                )
            } else {
                (
                    vec![*block_pair.s_block()],
                    block_pair.t_block().split_x().to_vec(),
                )
            }
        } else {
            if block_pair.s_block().width() < block_pair.s_block().height() {
                (
                    block_pair.s_block().split_y().to_vec(),
                    vec![*block_pair.t_block()],
                )
            } else {
                (
                    block_pair.s_block().split_x().to_vec(),
                    vec![*block_pair.t_block()],
                )
            }
        };

        let children = (
            children
                .0
                .into_iter()
                .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
            children
                .1
                .into_iter()
                .filter(|block| graph.locate_in_envelope(block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
        );
        children
            .0
            .into_iter()
            .flat_map(|s_block| {
                children.1.iter().map(move |t_block| {
                    BlockPair::new(
                        s_block,
                        *t_block,
                        block_pair.poi_id(),
                        block_pair.values().epsilon,
                        graph,
                    )
                })
            })
            .collect()
    }
}
