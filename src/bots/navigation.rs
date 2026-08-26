use crate::map::{
    MAP_CELL_SIZE_WORLD, MAX_MAP_DIMENSION_CELLS, MapDimensions, MapShape, ResolvedMap,
};
use bevy::prelude::*;
use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeSet, BinaryHeap},
};

const MAX_NAVIGATION_NODES: usize =
    MAX_MAP_DIMENSION_CELLS as usize * MAX_MAP_DIMENSION_CELLS as usize;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BotNavigationSnapshot {
    pub dimensions: MapDimensions,
    pub(super) blocked: BTreeSet<u32>,
}

#[derive(Clone, Debug)]
pub(super) struct BotRouteSearch {
    start: Vec2,
    start_index: u32,
    goal_index: u32,
    goal_cell: (u16, u16),
    blocked: BTreeSet<u32>,
    costs: Vec<u32>,
    parents: Vec<u32>,
    open: BinaryHeap<Reverse<OpenNode>>,
    expansions: usize,
    maximum_expansions: usize,
    maximum_points: usize,
}

#[derive(Clone, Debug)]
pub(super) enum BotRouteStart {
    Complete(Vec<Vec2>),
    Search(BotRouteSearch),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum BotRouteProgress {
    Pending,
    Complete(Vec<Vec2>),
    Exhausted,
}

impl BotNavigationSnapshot {
    pub(super) fn from_map(map: &ResolvedMap, clearance: f32) -> Option<Self> {
        let dimensions = map.snapshot.dimensions;
        let node_count = usize::from(dimensions.width) * usize::from(dimensions.height);
        if node_count == 0 || node_count > MAX_NAVIGATION_NODES || !clearance.is_finite() {
            return None;
        }
        let mut blocked = BTreeSet::new();
        for y in 0..dimensions.height {
            for x in 0..dimensions.width {
                let cell = crate::map::MapCell::new(x, y);
                let point = dimensions.cell_center(cell);
                if map.static_colliders.iter().any(|collider| {
                    shape_contains_with_clearance(
                        collider.position,
                        collider.shape,
                        point,
                        clearance,
                    )
                }) {
                    blocked.insert(index(dimensions, x, y));
                }
            }
        }
        Some(Self {
            dimensions,
            blocked,
        })
    }

    #[cfg(test)]
    pub(super) fn route(
        &self,
        start: Vec2,
        goal: Vec2,
        dynamic_blockers: &[Vec2],
        maximum_expansions: usize,
        maximum_points: usize,
    ) -> Option<Vec<Vec2>> {
        match self.begin_route(
            start,
            goal,
            dynamic_blockers,
            maximum_expansions,
            maximum_points,
        )? {
            BotRouteStart::Complete(route) => Some(route),
            BotRouteStart::Search(mut search) => match search.advance(self, maximum_expansions) {
                BotRouteProgress::Complete(route) => Some(route),
                BotRouteProgress::Pending | BotRouteProgress::Exhausted => None,
            },
        }
    }

    pub(super) fn begin_route(
        &self,
        start: Vec2,
        goal: Vec2,
        dynamic_blockers: &[Vec2],
        maximum_expansions: usize,
        maximum_points: usize,
    ) -> Option<BotRouteStart> {
        if !start.is_finite() || !goal.is_finite() || maximum_expansions == 0 {
            return None;
        }
        let goal = self.clamp_goal(goal);
        if self.line_clear(start, goal, dynamic_blockers) {
            return Some(BotRouteStart::Complete(vec![goal]));
        }
        let start_cell = self.world_to_cell(start)?;
        let goal_cell = self.world_to_cell(goal)?;
        let node_count = usize::from(self.dimensions.width) * usize::from(self.dimensions.height);
        let start_index = index(self.dimensions, start_cell.0, start_cell.1);
        let goal_index = index(self.dimensions, goal_cell.0, goal_cell.1);
        let mut blocked = self.blocked.clone();
        for blocker in dynamic_blockers
            .iter()
            .copied()
            .filter(|point| point.is_finite())
        {
            if let Some((x, y)) = self.world_to_cell(blocker) {
                blocked.insert(index(self.dimensions, x, y));
            }
        }
        blocked.remove(&start_index);
        blocked.remove(&goal_index);

        let mut costs = vec![u32::MAX; node_count];
        let parents = vec![u32::MAX; node_count];
        let mut open = BinaryHeap::new();
        costs[start_index as usize] = 0;
        open.push(Reverse(OpenNode {
            estimate: heuristic(start_cell, goal_cell),
            cost: 0,
            index: start_index,
        }));
        Some(BotRouteStart::Search(BotRouteSearch {
            start,
            start_index,
            goal_index,
            goal_cell,
            blocked,
            costs,
            parents,
            open,
            expansions: 0,
            maximum_expansions,
            maximum_points,
        }))
    }

    pub(super) fn line_clear(&self, start: Vec2, end: Vec2, dynamic_blockers: &[Vec2]) -> bool {
        if !start.is_finite() || !end.is_finite() {
            return false;
        }
        let distance = start.distance(end);
        let Some(direction) = (end - start).try_normalize() else {
            return true;
        };
        let mut traveled = MAP_CELL_SIZE_WORLD * 0.5;
        while traveled < distance {
            let point = start + direction * traveled;
            let Some((x, y)) = self.world_to_cell(point) else {
                return false;
            };
            if self.blocked.contains(&index(self.dimensions, x, y))
                || dynamic_blockers
                    .iter()
                    .any(|blocker| point.distance_squared(*blocker) <= 28.0_f32.powi(2))
            {
                return false;
            }
            traveled += MAP_CELL_SIZE_WORLD * 0.5;
        }
        true
    }

    pub(super) fn clamp_goal(&self, goal: Vec2) -> Vec2 {
        self.dimensions
            .bounds()
            .clamp_circle(goal, MAP_CELL_SIZE_WORLD)
    }

    pub(super) fn is_inside_perimeter(&self, point: Vec2, inset: f32) -> bool {
        self.dimensions.bounds().contains_with_inset(point, inset)
    }

    pub(super) fn perimeter_recovery_goal(&self, point: Vec2, inset: f32) -> Vec2 {
        self.dimensions.bounds().clamp_circle(point, inset)
    }

    pub(super) fn escape_axis(
        &self,
        start: Vec2,
        goal: Vec2,
        dynamic_blockers: &[Vec2],
        stable_variant: u64,
    ) -> Vec2 {
        let bounds = self.dimensions.bounds();
        let inward = (bounds.center() - start).try_normalize().unwrap_or(Vec2::X);
        let toward_goal = (goal - start).try_normalize().unwrap_or(inward);
        let directions = [
            Vec2::X,
            Vec2::Y,
            Vec2::NEG_X,
            Vec2::NEG_Y,
            Vec2::new(1.0, 1.0).normalize(),
            Vec2::new(-1.0, 1.0).normalize(),
            Vec2::new(-1.0, -1.0).normalize(),
            Vec2::new(1.0, -1.0).normalize(),
        ];
        let offset = usize::try_from(stable_variant % directions.len() as u64).unwrap_or(0);
        (0..directions.len())
            .filter_map(|ordinal| {
                let direction = directions[(ordinal + offset) % directions.len()];
                let endpoint = start + direction * (MAP_CELL_SIZE_WORLD * 2.0);
                (bounds.contains_with_inset(endpoint, MAP_CELL_SIZE_WORLD * 0.5)
                    && self.line_clear(start, endpoint, dynamic_blockers))
                .then_some((
                    direction,
                    direction.dot(inward) + direction.dot(toward_goal) * 0.25,
                ))
            })
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map_or(inward, |(direction, _)| direction)
    }

    fn reconstruct(
        &self,
        parents: &[u32],
        start: u32,
        goal: u32,
        maximum_points: usize,
    ) -> Option<Vec<Vec2>> {
        let mut cursor = goal;
        let mut reversed = Vec::new();
        while cursor != start {
            if reversed.len() >= maximum_points {
                return None;
            }
            let (x, y) = coordinates(self.dimensions, cursor);
            reversed.push(self.dimensions.cell_center(crate::map::MapCell::new(x, y)));
            cursor = *parents.get(cursor as usize)?;
            if cursor == u32::MAX {
                return None;
            }
        }
        reversed.reverse();
        Some(reversed)
    }

    fn world_to_cell(&self, point: Vec2) -> Option<(u16, u16)> {
        let bounds = self.dimensions.bounds();
        if !bounds.contains(point) {
            return None;
        }
        let local = (point - bounds.min) / MAP_CELL_SIZE_WORLD;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cell = (local.x.floor() as u16, local.y.floor() as u16);
        (cell.0 < self.dimensions.width && cell.1 < self.dimensions.height).then_some(cell)
    }
}

impl BotRouteSearch {
    pub(super) fn expansions(&self) -> usize {
        self.expansions
    }

    pub(super) fn advance(
        &mut self,
        navigation: &BotNavigationSnapshot,
        expansion_budget: usize,
    ) -> BotRouteProgress {
        let mut tick_expansions = 0;
        while let Some(Reverse(current)) = self.open.pop() {
            if current.cost != self.costs[current.index as usize] {
                continue;
            }
            if current.index == self.goal_index {
                let Some(route) = navigation.reconstruct(
                    &self.parents,
                    self.start_index,
                    self.goal_index,
                    self.maximum_points,
                ) else {
                    return BotRouteProgress::Exhausted;
                };
                return BotRouteProgress::Complete(compress_collinear_route(self.start, route));
            }
            if tick_expansions >= expansion_budget {
                self.open.push(Reverse(current));
                return BotRouteProgress::Pending;
            }
            if self.expansions >= self.maximum_expansions {
                return BotRouteProgress::Exhausted;
            }
            tick_expansions += 1;
            self.expansions += 1;
            let (x, y) = coordinates(navigation.dimensions, current.index);
            for (dx, dy, step_cost) in NEIGHBORS {
                let Some(nx) = x.checked_add_signed(dx) else {
                    continue;
                };
                let Some(ny) = y.checked_add_signed(dy) else {
                    continue;
                };
                if nx >= navigation.dimensions.width || ny >= navigation.dimensions.height {
                    continue;
                }
                let next = index(navigation.dimensions, nx, ny);
                if self.blocked.contains(&next)
                    || (dx != 0
                        && dy != 0
                        && (self.blocked.contains(&index(navigation.dimensions, nx, y))
                            || self.blocked.contains(&index(navigation.dimensions, x, ny))))
                {
                    continue;
                }
                let next_cost = current.cost.saturating_add(step_cost);
                if next_cost >= self.costs[next as usize] {
                    continue;
                }
                self.costs[next as usize] = next_cost;
                self.parents[next as usize] = current.index;
                self.open.push(Reverse(OpenNode {
                    estimate: next_cost.saturating_add(heuristic((nx, ny), self.goal_cell)),
                    cost: next_cost,
                    index: next,
                }));
            }
        }
        BotRouteProgress::Exhausted
    }
}

fn compress_collinear_route(start: Vec2, route: Vec<Vec2>) -> Vec<Vec2> {
    if route.len() < 2 {
        return route;
    }
    let mut compressed = Vec::with_capacity(route.len());
    let mut previous_direction = (route[0] - start).try_normalize();
    for window in route.windows(2) {
        let direction = (window[1] - window[0]).try_normalize();
        if direction != previous_direction {
            compressed.push(window[0]);
        }
        previous_direction = direction;
    }
    compressed.push(*route.last().expect("nonempty route"));
    compressed
}

fn shape_contains_with_clearance(
    center: Vec2,
    shape: MapShape,
    point: Vec2,
    clearance: f32,
) -> bool {
    match shape {
        MapShape::Rectangle { half_extents } => {
            let delta = (point - center).abs();
            delta.x <= half_extents.x + clearance && delta.y <= half_extents.y + clearance
        }
        MapShape::Circle { radius } => {
            point.distance_squared(center) <= (radius + clearance).powi(2)
        }
    }
}

fn index(dimensions: MapDimensions, x: u16, y: u16) -> u32 {
    u32::from(y) * u32::from(dimensions.width) + u32::from(x)
}

fn coordinates(dimensions: MapDimensions, index: u32) -> (u16, u16) {
    let width = u32::from(dimensions.width);
    (
        u16::try_from(index % width).expect("validated navigation x fits u16"),
        u16::try_from(index / width).expect("validated navigation y fits u16"),
    )
}

fn heuristic(from: (u16, u16), to: (u16, u16)) -> u32 {
    let dx = u32::from(from.0.abs_diff(to.0));
    let dy = u32::from(from.1.abs_diff(to.1));
    let diagonal = dx.min(dy);
    diagonal * 1_414 + (dx.max(dy) - diagonal) * 1_000
}

const NEIGHBORS: [(i16, i16, u32); 8] = [
    (-1, 0, 1_000),
    (0, -1, 1_000),
    (0, 1, 1_000),
    (1, 0, 1_000),
    (-1, -1, 1_414),
    (-1, 1, 1_414),
    (1, -1, 1_414),
    (1, 1, 1_414),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OpenNode {
    estimate: u32,
    cost: u32,
    index: u32,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.estimate, self.cost, self.index).cmp(&(other.estimate, other.cost, other.index))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod clearance_tests {
    use super::*;

    #[test]
    fn one_unit_passage_remains_open_with_bot_safety_allowance() {
        let point = Vec2::new(16.0, 0.0);
        let wall = MapShape::Rectangle {
            half_extents: Vec2::splat(16.0),
        };
        let clearance = crate::movement::STANDARD_FIGHTER_RADIUS + 1.0;

        assert!(!shape_contains_with_clearance(
            Vec2::new(-16.0, 0.0),
            wall,
            point,
            clearance
        ));
        assert!(!shape_contains_with_clearance(
            Vec2::new(48.0, 0.0),
            wall,
            point,
            clearance
        ));
        assert!(shape_contains_with_clearance(
            Vec2::new(-16.0, 0.0),
            wall,
            point,
            16.0
        ));
    }
}
