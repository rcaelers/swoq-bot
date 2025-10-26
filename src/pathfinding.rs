use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::map::Map;
use crate::types::Position;

#[derive(Clone, Eq, PartialEq)]
struct Node {
    pos: Position,
    f_score: i32,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_score.cmp(&self.f_score)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct AStar;

impl AStar {
    #[tracing::instrument(level = "trace", skip(map, is_walkable), fields(start_x = start.x, start_y = start.y, goal_x = goal.x, goal_y = goal.y))]
    pub fn find_path<F>(
        map: &Map,
        start: Position,
        goal: Position,
        is_walkable: F,
    ) -> Option<Vec<Position>>
    where
        F: Fn(&Position, Position) -> bool,
    {
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<Position, Position> = HashMap::new();
        let mut g_score: HashMap<Position, i32> = HashMap::new();
        let mut closed_set: HashSet<Position> = HashSet::new();

        g_score.insert(start, 0);
        open_set.push(Node {
            pos: start,
            f_score: heuristic(start, goal),
        });

        // Limit node expansions to prevent excessive searching for unreachable targets
        const MAX_EXPANSIONS: usize = 5000;
        let mut expansions = 0;

        while let Some(Node { pos: current, .. }) = open_set.pop() {
            if current == goal {
                tracing::trace!(expansions, "Path found");
                return Some(reconstruct_path(&came_from, current));
            }

            if closed_set.contains(&current) {
                continue;
            }
            closed_set.insert(current);

            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                // Too many expansions, target likely unreachable
                tracing::warn!(expansions, "Max expansions reached, target unreachable");
                return None;
            }

            for neighbor in current.neighbors() {
                if closed_set.contains(&neighbor) {
                    continue;
                }

                // Check if neighbor is within bounds
                if neighbor.x < 0
                    || neighbor.x >= map.width
                    || neighbor.y < 0
                    || neighbor.y >= map.height
                {
                    continue;
                }

                if !is_walkable(&neighbor, goal) {
                    continue;
                }

                let tentative_g = g_score.get(&current).unwrap_or(&i32::MAX) + 1;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    open_set.push(Node {
                        pos: neighbor,
                        f_score: tentative_g + heuristic(neighbor, goal),
                    });
                }
            }
        }

        tracing::trace!(expansions, "No path found");
        None
    }

    /// Find a path while avoiding collisions with another player's path at matching ticks.
    /// The callback `is_walkable_at_tick` receives (position, goal, tick_from_start).
    #[tracing::instrument(level = "trace", skip(map, is_walkable_at_tick), fields(start_x = start.x, start_y = start.y, goal_x = goal.x, goal_y = goal.y))]
    pub fn find_path_with_tick<F>(
        map: &Map,
        start: Position,
        goal: Position,
        is_walkable_at_tick: F,
    ) -> Option<Vec<Position>>
    where
        F: Fn(&Position, Position, i32) -> bool,
    {
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<Position, Position> = HashMap::new();
        let mut g_score: HashMap<Position, i32> = HashMap::new();
        let mut closed_set: HashSet<Position> = HashSet::new();

        g_score.insert(start, 0);
        open_set.push(Node {
            pos: start,
            f_score: heuristic(start, goal),
        });

        const MAX_EXPANSIONS: usize = 5000;
        let mut expansions = 0;

        while let Some(Node { pos: current, .. }) = open_set.pop() {
            if current == goal {
                tracing::trace!(expansions, "Path found");
                return Some(reconstruct_path(&came_from, current));
            }

            if closed_set.contains(&current) {
                continue;
            }
            closed_set.insert(current);

            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                tracing::warn!(expansions, "Max expansions reached, target unreachable");
                return None;
            }

            let current_tick = *g_score.get(&current).unwrap_or(&0);

            for neighbor in current.neighbors() {
                if closed_set.contains(&neighbor) {
                    continue;
                }

                if neighbor.x < 0
                    || neighbor.x >= map.width
                    || neighbor.y < 0
                    || neighbor.y >= map.height
                {
                    continue;
                }

                let next_tick = current_tick + 1;
                if !is_walkable_at_tick(&neighbor, goal, next_tick) {
                    continue;
                }

                let tentative_g = current_tick + 1;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    open_set.push(Node {
                        pos: neighbor,
                        f_score: tentative_g + heuristic(neighbor, goal),
                    });
                }
            }
        }

        tracing::trace!(expansions, "No path found");
        None
    }
}

fn heuristic(a: Position, b: Position) -> i32 {
    a.distance(&b)
}

fn reconstruct_path(
    came_from: &HashMap<Position, Position>,
    mut current: Position,
) -> Vec<Position> {
    let mut path = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        path.push(prev);
        current = prev;
    }
    path.reverse();
    path
}
