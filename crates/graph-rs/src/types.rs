use geo::{Coord, CoordNum};
use num_traits::{Num, NumOps};

use crate::{CoordGraph, graph::Target};

#[derive(Clone, Copy)]
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

pub struct Path<EV> {
    nodes: Vec<Target<EV>>,
}

impl<EV: Num + Copy> Path<EV> {
    pub fn cost(&self) -> EV {
        self.nodes
            .iter()
            .fold(EV::zero(), |acc, e| acc + *e.value())
    }
}
