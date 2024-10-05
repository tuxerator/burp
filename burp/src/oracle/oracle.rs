use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    fmt::Debug,
    iter::Peekable,
};

use graph_rs::{
    algorithms::dijkstra::Dijkstra, graph::quad_tree::QuadGraph, types::Direction, Coordinate,
    DirectedGraph,
};
use log::{info, warn};
use ordered_float::FloatCore;
use qutee::{Boundary, DynCap, Point, QueryPoints};

pub struct Oracle {
    b_tree: BTreeMap<usize, usize>,
}

impl Oracle {
    fn new() {}
}

fn build<EV, NV, G>(graph: QuadGraph<EV, NV, G>, poi: usize, epsilon: EV)
where
    EV: FloatCore + Default + Debug + Copy + Send + Sync,
    NV: Coordinate + Debug,
    G: DirectedGraph<EV, NV>,
{
    let root = graph.boundary().clone();
    let mut queue = VecDeque::new();

    let mut result = vec![];

    queue.push_back((root, root));

    while let Some(block) = queue.pop_front() {
        let mut points = (
            graph.query_points(block.0).peekable(),
            graph.query_points(block.1).peekable(),
        );
        let (Some(s), Some(t)) = (points.0.peek(), points.1.peek()) else {
            info!("At least one block is empty. {:?}", &block);
            continue;
        };

        let values = Values {
            d_st: graph
                .dijkstra(s.1, HashSet::from([t.1]), Direction::Outgoing)
                .unwrap()
                .get(t.1)
                .unwrap()
                .cost()
                .0,
            d_sp: graph
                .dijkstra(s.1, HashSet::from([poi]), Direction::Outgoing)
                .unwrap()
                .get(poi)
                .unwrap()
                .cost()
                .0,
            d_pt: graph
                .dijkstra(poi, HashSet::from([t.1]), Direction::Outgoing)
                .unwrap()
                .get(t.1)
                .unwrap()
                .cost()
                .0,
            r_af: graph.raduis(s.1, &block.0, Direction::Outgoing).unwrap(),
            r_ab: graph.raduis(s.1, &block.0, Direction::Incoming).unwrap(),
            r_bf: graph.raduis(t.1, &block.1, Direction::Outgoing).unwrap(),
            r_bb: graph.raduis(t.1, &block.1, Direction::Incoming).unwrap(),
        };

        if values.in_path(epsilon) {
            result.push(block);
        } else if values.not_in_path(epsilon) {
            continue;
        }

        let children = (divide(&block.0).into_iter(), divide(&block.1).into_iter());

        let children = (
            children
                .0
                .filter(|block| graph.query(*block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
            children
                .1
                .filter(|block| graph.query(*block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
        );
        let mut child_block_pairs: Vec<_> = children
            .0
            .into_iter()
            .flat_map(|block_a| {
                children
                    .1
                    .iter()
                    .map(move |block_b| (block_a, block_b.clone()))
            })
            .collect();

        queue.append(&mut child_block_pairs.into());
    }
}

struct Values<T: FloatCore> {
    d_st: T,
    d_sp: T,
    d_pt: T,
    r_af: T,
    r_ab: T,
    r_bf: T,
    r_bb: T,
}

impl<T: FloatCore> Values<T> {
    fn in_path(&self, epsilon: T) -> bool {
        (self.r_ab + self.d_sp + self.d_pt + self.r_bf) / (self.d_st - (self.r_af + self.r_bb))
            - T::from(1).unwrap()
            <= epsilon
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt - (self.r_ab + self.r_bf)) / (self.d_st + (self.r_ab + self.r_bf))
            - T::from(1).unwrap()
            >= epsilon
    }
}

fn divide<C: qutee::Coordinate>(block: &Boundary<C>) -> [Boundary<C>; 4] {
    let half_block_height = block.height()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let half_block_width = block.width()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let minus_one = C::from(-1).unwrap_or_else(|| {
        panic!(
            "Could not convert '-1' into type '{}'",
            std::any::type_name::<C>()
        )
    });
    let top_left = block.top_left();
    let corner_points = [
        *top_left,
        Point::new(top_left.x + half_block_width, top_left.y),
        Point::new(top_left.x, top_left.y - half_block_height),
        Point::new(
            top_left.x + half_block_width,
            top_left.y - half_block_height,
        ),
    ];

    corner_points
        .into_iter()
        .map(|corner| Boundary::new(corner, half_block_width, half_block_height * minus_one))
        .collect::<Vec<Boundary<C>>>()
        .try_into()
        .unwrap_or_else(|_| {
            panic!(
                "Could not convert 'Vec<Boundary<{}> to '[Boundary<{}>; 4]'",
                std::any::type_name::<C>(),
                std::any::type_name::<C>()
            )
        })
}

#[cfg(test)]
mod test {
    use std::ops::Bound;

    use qutee::{Boundary, Point};

    use super::divide;

    #[test]
    fn corner_points_test() {
        let block = Boundary::new(Point::new(-0.5, 0.5), 1.0, -1.0);
        dbg!(block);
        let split = divide(&block);
        let expected = [
            Boundary::new(Point::new(-0.5, 0.5), 0.5, -0.5),
            Boundary::new(Point::new(0.0, 0.5), 0.5, -0.5),
            Boundary::new(Point::new(-0.5, 0.0), 0.5, -0.5),
            Boundary::new(Point::new(0.0, 0.0), 0.5, -0.5),
        ];

        assert_eq!(split, expected);
    }
}
