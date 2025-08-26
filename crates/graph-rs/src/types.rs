use geo::{Coord, CoordNum};
use num_traits::{Num, NumOps};

use crate::{CoordGraph, graph::Target};

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum Direction {
    Outgoing,
    Incoming,
    Undirected,
}

pub struct CoordNode<C, NV>
where
    C: CoordNum,
{
    coord: Coord<C>,
    data: NV,
}
