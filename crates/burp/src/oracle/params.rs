use std::fmt::Debug;

use crate::oracle::{
    MinimalSplitStrategy, SimpleSplitStrategy, SplitStrategy as SplitStrategyTrait,
};

pub trait OracleParams: Copy + Clone + Debug + Default {
    /// The split strategy which is used for spliting the block pairs.
    type SplitStrategy: SplitStrategyTrait;

    /// Wheater to merge blocks into their parents if they all are in-path.
    const MERGE_BLOCKS: bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct DefaultOracleParams;

impl OracleParams for DefaultOracleParams {
    type SplitStrategy = SimpleSplitStrategy;

    const MERGE_BLOCKS: bool = true;
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct MinSplitParams;

impl OracleParams for MinSplitParams {
    type SplitStrategy = MinimalSplitStrategy;

    const MERGE_BLOCKS: bool = true;
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct NoMergeParams;

impl OracleParams for NoMergeParams {
    type SplitStrategy = SimpleSplitStrategy;

    const MERGE_BLOCKS: bool = false;
}
