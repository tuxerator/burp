use std::{
    collections::{HashSet, VecDeque},
    fmt::Debug,
    hash::RandomState,
};

use geo::{Coord, CoordFloat, Rect};
use graph_rs::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    graph::{quad_tree::QuadGraph, rstar::RTreeGraph},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use indicatif::ProgressBar;
use log::debug;
use num_traits::Num;
use ordered_float::FloatCore;
use qutee::{Boundary, DynCap, Point, QueryPoints};
use rstar::{
    primitives::{GeomWithData, ObjectRef, Rectangle},
    RTree, RTreeNum, RTreeObject, RTreeParams, AABB,
};

macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Some(x) => x,
            None => continue,
        }
    };
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct BlockPair<C>
where
    C: RTreeNum + CoordFloat,
{
    pub s_block: Rectangle<Coord<C>>,
    pub t_block: Rectangle<Coord<C>>,
    pub poi_id: usize,
}

pub struct Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    r_tree: RTree<GeomWithData<Rectangle<Coord<C>>, usize>>,
    block_pairs: Vec<BlockPair<C>>,
}

impl<C> Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    pub fn new() -> Self {
        Oracle {
            r_tree: RTree::new(),
            block_pairs: vec![],
        }
    }

    fn add_block_pair(&mut self, s_block: Rect<C>, t_block: Rect<C>, poi: usize) -> BlockPair<C> {
        let s_geom = Rectangle::from_corners(s_block.min(), s_block.max());

        let t_geom = Rectangle::from_corners(t_block.min(), t_block.max());

        let block_pair = BlockPair {
            s_block: s_geom,
            t_block: t_geom,
            poi_id: poi,
        };

        self.block_pairs.push(block_pair);

        let block_pair = self.block_pairs.last().expect("block_pairs is empty");

        let s_rect = GeomWithData::new(block_pair.s_block, self.block_pairs.len() - 1);
        let t_rect = GeomWithData::new(block_pair.t_block, self.block_pairs.len() - 1);

        self.r_tree.insert(s_rect);
        self.r_tree.insert(t_rect);

        *block_pair
    }

    pub fn get_block_pairs(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> Vec<&BlockPair<C>> {
        let s_blocks = self
            .r_tree
            .locate_all_at_point(s_coord)
            .map(|geom| geom.data);
        let t_blocks = self
            .r_tree
            .locate_all_at_point(t_coord)
            .map(|geom| geom.data);

        let s_blocks = HashSet::<_, RandomState>::from_iter(s_blocks);
        let t_blocks = HashSet::<_, RandomState>::from_iter(t_blocks);

        let block_pair_ids = HashSet::<_, RandomState>::from_iter(s_blocks.intersection(&t_blocks));

        self.block_pairs
            .iter()
            .enumerate()
            .filter(|(i, _)| block_pair_ids.contains(i))
            .map(|e| e.1)
            .collect()
    }

    pub fn get_pois(&self, s_coord: &Coord<C>, t_coord: &Coord<C>) -> Vec<usize> {
        let block_pairs = self.get_block_pairs(s_coord, t_coord);

        block_pairs.into_iter().map(|b| b.poi_id).collect()
    }

    pub fn get_blocks_at(&self, coord: &Coord<C>) -> Vec<&BlockPair<C>> {
        let block_pairs = self.r_tree.locate_all_at_point(coord).map(|geom| geom.data);

        let block_pairs = HashSet::<_, RandomState>::from_iter(block_pairs);

        self.block_pairs
            .iter()
            .enumerate()
            .filter(|(i, _)| block_pairs.contains(i))
            .map(|e| e.1)
            .collect()
    }

    pub fn build<EV, NV, G>(
        graph: &mut RTreeGraph<EV, NV, G, C>,
        poi: usize,
        epsilon: EV,
    ) -> Result<Self, String>
    where
        EV: FloatCore,
        NV: Coordinate<C> + Debug,
        G: DirectedGraph<EV, NV> + CachedDijkstra<EV, NV>,
    {
        // let progress_bar = ProgressBar::new( graph .node_count()
        //         .try_into()
        //         .expect("Value doesn't fit in u64"),
        // );

        let mut oracle = Oracle::<C>::new();

        let Some(root) = graph.bounding_rect() else {
            return Err("Could not get bounding rect of graph".to_string());
        };
        let mut queue = VecDeque::new();

        queue.push_back((root, root));

        while let Some(block_pair) = queue.pop_front() {
            if block_pair.0 != block_pair.1 {
                let s: GeomWithData<Coord<C>, usize>;
                let t: GeomWithData<Coord<C>, usize>;
                {
                    let mut points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .cloned(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .cloned(),
                    );
                    let (Some(p_0), Some(p_1)) = (points.0.next(), points.1.next()) else {
                        debug!("At least one block is empty. {:?}", &block_pair);
                        continue;
                    };

                    s = p_0;
                    t = p_1;
                }

                let Some(values) = Values::new(graph, s, t, block_pair, poi) else {
                    continue;
                };

                if values.in_path(epsilon) {
                    debug!("Found in-path block pair: {:#?}", &block_pair);
                    oracle.add_block_pair(block_pair.0, block_pair.1, poi);
                    // progress_bar.inc(
                    //     (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                    //         .try_into()
                    //         .expect("value doesn't fit in u64"),
                    // );

                    continue;
                } else if values.not_in_path(epsilon) {
                    debug!("Found not in-path block pair: {:#?}", &block_pair);
                    // progress_bar.inc(
                    //     (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                    //         .try_into()
                    //         .expect("value doesn't fit in u64"),
                    // );
                    continue;
                }
            }

            let children = (
                block_pair
                    .0
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
                block_pair
                    .1
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
            );

            let children = (
                children
                    .0
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
                children
                    .1
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
            );
            let child_block_pairs: Vec<_> = children
                .0
                .into_iter()
                .flat_map(|block_a| children.1.iter().map(move |block_b| (block_a, *block_b)))
                .filter(|block_pair| {
                    let points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .collect::<Vec<_>>(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .collect::<Vec<_>>(),
                    );

                    // If both blocks only contain the same node they can be discarded
                    if points.0.len() == 1 && points.0 == points.1 {
                        return false;
                    }
                    true
                })
                .collect();

            debug!("inserting {} block pairs", child_block_pairs.len());

            queue.append(&mut child_block_pairs.into());
        }

        Ok(oracle)
    }
}

impl<C> Default for Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    fn default() -> Self {
        Self::new()
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
    fn new<NV, G, C>(
        graph: &mut RTreeGraph<T, NV, G, C>,
        s: GeomWithData<Coord<C>, usize>,
        t: GeomWithData<Coord<C>, usize>,
        block_pair: (Rect<C>, Rect<C>),
        poi: usize,
    ) -> Option<Self>
    where
        NV: Coordinate<C> + Debug,
        G: DirectedGraph<T, NV> + CachedDijkstra<T, NV>,
        C: RTreeNum + CoordFloat,
    {
        Some(Values {
            d_st: *graph
                .cached_dijkstra(s.data, HashSet::from([t.data]), Direction::Outgoing)
                .unwrap()
                .get(t.data)?
                .cost(),
            d_sp: *graph
                .cached_dijkstra(s.data, HashSet::from([poi]), Direction::Outgoing)
                .unwrap()
                .get(poi)?
                .cost(),
            d_pt: *graph
                .cached_dijkstra(poi, HashSet::from([t.data]), Direction::Outgoing)
                .unwrap()
                .get(t.data)?
                .cost(),
            r_af: graph
                .radius(
                    s.data,
                    &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                    Direction::Outgoing,
                )
                .unwrap(),
            r_ab: graph
                .radius(
                    s.data,
                    &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                    Direction::Incoming,
                )
                .unwrap(),
            r_bf: graph
                .radius(
                    t.data,
                    &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                    Direction::Outgoing,
                )
                .unwrap(),
            r_bb: graph
                .radius(
                    t.data,
                    &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                    Direction::Incoming,
                )
                .unwrap(),
        })
    }

    fn in_path(&self, epsilon: T) -> bool {
        (self.r_ab + self.d_sp + self.d_pt + self.r_bf)
            <= (self.d_st - (self.r_af + self.r_bb)) * (T::from(1).unwrap() + epsilon)
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt - (self.r_af + self.r_bb))
            >= (self.d_st + (self.r_ab + self.r_bf)) * (T::from(1).unwrap() + epsilon)
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
    use std::{f64, ops::Bound};

    use geo::Rect;
    use qutee::{Area, Boundary, Point};
    use rand::random;

    use super::{divide, BlockPair, Oracle};

    #[test]
    fn corner_points_test() {
        for _ in 0..1000 {
            let point: Point<f64> = Point::new(
                random::<f64>() * 360.0 - 180.0,
                random::<f64>() * 180.0 - 90.0,
            );
            let block = Boundary::new(point, random::<f64>() * 360.0, -random::<f64>() * 180.0);
            let split = divide(&block);
            let expected = [
                Boundary::new(point, block.width() / 2.0, -block.height() / 2.0),
                Boundary::new(
                    Point::new(point.x + block.width() / 2.0, point.y),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(point.x, point.y - block.height() / 2.0),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(
                        point.x + block.width() / 2.0,
                        point.y - block.height() / 2.0,
                    ),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
            ];

            assert_eq!(split, expected);
            for i in 0..4 {
                for j in 0..4 {
                    if i != j {
                        assert!(!split[i].intersects(&split[j]));
                    }
                }
            }
        }
    }

    #[test]
    fn add_block_pair_test() {
        let mut oracle = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        oracle.add_block_pair(rect_0, rect_1, 0);
    }

    #[test]
    fn get_block_pairs_test() {
        let mut oracle = Oracle::default();

        let rect_0 = Rect::new((0.5, 0.5), (1.0, 1.0));
        let rect_1 = Rect::new((10., 9.), (11., 12.));

        let expected = oracle.add_block_pair(rect_0, rect_1, 0);

        let block_pairs = oracle.get_block_pairs(&(0.6, 0.8).into(), &(10.5, 10.).into());

        assert_eq!(block_pairs, vec![&expected]);
    }
}
