use std::{
    cell::RefCell,
    cmp::max,
    fmt::{Debug, Display},
    marker::PhantomData,
    rc::Rc,
};

use geo::{Coord, CoordFloat, Rect};
use graph_rs::{
    CoordGraph, Graph,
    algorithms::dijkstra::{Dijkstra, ResultNode},
    types::Direction,
};
use log::trace;
use ordered_float::{FloatCore, OrderedFloat};
use rstar::{RTreeNum, primitives::Rectangle};
use rustc_hash::FxHashSet;
use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeOwned, Visitor},
    ser::SerializeStruct,
};

use crate::oracle::oracle::Radius;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockPair<EV, C>
where
    EV: FloatCore,
    C: RTreeNum + CoordFloat,
{
    s_block: Rect<C>,
    t_block: Rect<C>,
    poi_id: usize,
    values: Values<EV>,
}

impl<EV, C> BlockPair<EV, C>
where
    EV: FloatCore,
    C: RTreeNum + CoordFloat,
{
    pub fn new<G>(s_block: Rect<C>, t_block: Rect<C>, poi_id: usize, graph: &G) -> Self
    where
        G: CoordGraph<C = C, EV = EV> + Dijkstra + Radius,
    {
        let values = Values::new(poi_id, &s_block, &t_block, graph).unwrap();
        BlockPair {
            s_block,
            t_block,
            poi_id,
            values,
        }
    }

    pub fn s_block(&self) -> &Rect<C> {
        &self.s_block
    }

    pub fn t_block(&self) -> &Rect<C> {
        &self.t_block
    }

    pub fn poi_id(&self) -> usize {
        self.poi_id
    }

    pub fn s_block_as_rectangle(&self) -> Rectangle<Coord<C>> {
        Rectangle::from_corners(self.s_block.min(), self.s_block.max())
    }

    pub fn t_block_as_rectangle(&self) -> Rectangle<Coord<C>> {
        Rectangle::from_corners(self.t_block.min(), self.t_block.max())
    }

    pub fn values(&self) -> &Values<EV> {
        &self.values
    }
}

impl<EV, C> PartialEq for BlockPair<EV, C>
where
    EV: FloatCore + Debug,
    C: RTreeNum + CoordFloat + Debug,
{
    fn eq(&self, other: &Self) -> bool {
        (self.s_block == other.s_block)
            && (self.t_block == other.t_block)
            && (self.poi_id == other.poi_id)
    }
}

impl<EV, C> Display for BlockPair<EV, C>
where
    EV: FloatCore + Debug,
    C: RTreeNum + CoordFloat + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "s_block: {:#?}
            t_block: {:#?}
            poi_id: {}
            Values: \n{:>4}",
            self.s_block,
            self.t_block,
            self.poi_id,
            format!("{:?}", self.values),
        )
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Values<T: FloatCore> {
    pub d_st: T,
    pub d_sp: T,
    pub d_pt: T,
    pub r_af: Vec<ResultNode<T>>,
    pub r_ab: Vec<ResultNode<T>>,
    pub r_bf: Vec<ResultNode<T>>,
    pub r_bb: Vec<ResultNode<T>>,
}

impl<T: FloatCore> Values<T> {
    fn new<G>(
        poi_id: usize,
        s_block: &Rect<G::C>,
        t_block: &Rect<G::C>,
        graph: &G,
    ) -> Option<Values<G::EV>>
    where
        G: CoordGraph<EV = T> + Dijkstra + Radius,
        G::C: RTreeNum + CoordFloat,
    {
        let s: usize;
        let t: usize;
        {
            let mut points = (
                graph.locate_in_envelope(s_block),
                graph.locate_in_envelope(t_block),
            );

            let (Some(p_0), Some(p_1)) = (points.0.next(), points.1.next()) else {
                panic!("Found empty block! This is a bug in the splitting operation");
            };

            trace!("A-Points: {:?}", points.0.count() + 1);
            trace!("B-Points: {:?}", points.1.count() + 1);

            s = p_0;
            t = p_1;
        }
        let d_s = graph.dijkstra(s, FxHashSet::from_iter([t, poi_id]), Direction::Outgoing);
        Some(Values {
            d_st: *d_s.get(t)?.cost(),
            d_sp: *d_s.get(poi_id)?.cost(),
            d_pt: *graph
                .dijkstra(poi_id, FxHashSet::from_iter([t]), Direction::Outgoing)
                .get(t)?
                .cost(),
            r_af: graph.radius(s, s_block, Direction::Outgoing).unwrap(),
            r_ab: graph.radius(s, s_block, Direction::Incoming).unwrap(),
            r_bf: graph.radius(t, t_block, Direction::Outgoing).unwrap(),
            r_bb: graph.radius(t, t_block, Direction::Incoming).unwrap(),
        })
    }

    pub fn in_path(&self, epsilon: T) -> bool {
        (self.r_ab.last().map_or(T::zero(), |e| *e.cost())
            + self.d_sp
            + self.d_pt
            + self.r_bf.last().map_or(T::zero(), |e| *e.cost()))
            <= (self.d_st
                - (self.r_af.last().map_or(T::zero(), |e| *e.cost())
                    + self.r_bb.last().map_or(T::zero(), |e| *e.cost())))
                * (T::from(1).unwrap() + epsilon)
    }

    pub fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt
            - (self.r_af.last().map_or(T::zero(), |e| *e.cost())
                + self.r_bb.last().map_or(T::zero(), |e| *e.cost())))
            >= (self.d_st
                + (self.r_ab.last().map_or(T::zero(), |e| *e.cost())
                    + self.r_bf.last().map_or(T::zero(), |e| *e.cost())))
                * (T::from(1).unwrap() + epsilon)
    }
}

impl<T: FloatCore + Debug> Display for Values<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "d_st: {:?}\n
            d_sp: {:?}\n
            d_pt: {:?}\n
            r_af: {:?}\n
            r_ab: {:?}\n
            r_bf: {:?}\n
            r_bb: {:?}\n",
            self.d_st, self.d_sp, self.d_pt, self.r_af, self.r_ab, self.r_bf, self.r_bb
        )
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;

    use geo::{Coord, Rect};
    use graph_rs::{
        graph::{csr::DirectedCsrGraph, rstar::RTreeGraph},
        types::CoordNode,
    };
    use serde::{Deserialize, Serialize};
    use serde_test::{Token, assert_ser_tokens};

    use crate::oracle::block_pair::BlockPair;

    #[test]
    fn test_ser_de() {
        let graph: RTreeGraph<DirectedCsrGraph<f64, Coord>, f64> = RTreeGraph::default();
        let block_pair = BlockPair::<_, f64>::new(
            Rect::new((0.5, 0.5), (1.0, 1.0)),
            Rect::new((10., 9.), (11., 12.)),
            0,
            &graph,
        );

        let json = serde_json::to_string(&block_pair).unwrap();

        let block_pair_de = serde_json::from_str(json.as_str()).unwrap();

        assert_eq!(block_pair, block_pair_de);
    }
}
