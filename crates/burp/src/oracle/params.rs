use std::fmt::Debug;

use crate::oracle::{
    MinimalSplitStrategy, SimpleSplitStrategy, SplitStrategy as SplitStrategyTrait,
};

pub trait OracleParams: Copy + Clone + Debug + Default {
    /// The split strategy which is used for spliting the block pairs.
    type SplitStrategy: SplitStrategyTrait;

    /// Wheater to merge blocks into their parents if they all are in-path.
    fn merge_blocks(&self) -> bool;
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct DefaultOracleParams {
    pub merge_blocks: bool,
}

impl OracleParams for DefaultOracleParams {
    type SplitStrategy = SimpleSplitStrategy;

    fn merge_blocks(&self) -> bool {
        self.merge_blocks
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct MinSplitParams {
    pub merge_blocks: bool,
}

impl OracleParams for MinSplitParams {
    type SplitStrategy = MinimalSplitStrategy;

    fn merge_blocks(&self) -> bool {
        self.merge_blocks
    }
}
