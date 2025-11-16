use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::infra::Position;
use crate::state::Map;

#[derive(Clone, Eq, PartialEq)]
struct Node {
    pos: Position,
    f_score: i32,
    tick: i32, // Actual step count (not affected by weighted costs)
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
    /// Find a path with custom cost function for each step.
    /// The cost function receives (position, goal, tick_from_start) and returns the cost to enter that position.
    pub fn find_path_with_cost<F, C>(
        map: &Map,
        start: Position,
        goal: Position,
        is_walkable_at_tick: F,
        cost_fn: C,
    ) -> Option<Vec<Position>>
    where
        F: Fn(&Position, Position, i32) -> bool,
        C: Fn(&Position, Position, i32) -> i32,
    {
        // If start equals goal, return path with just the goal position
        if start == goal {
            return Some(vec![goal]);
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<Position, Position> = HashMap::new();
        let mut g_score: HashMap<Position, i32> = HashMap::new();
        let mut tick_map: HashMap<Position, i32> = HashMap::new(); // Track actual step count
        let mut closed_set: HashSet<Position> = HashSet::new();

        g_score.insert(start, 0);
        tick_map.insert(start, 0);
        open_set.push(Node {
            pos: start,
            f_score: heuristic(start, goal),
            tick: 0,
        });

        const MAX_EXPANSIONS: usize = 5000;
        let mut expansions = 0;

        while let Some(Node { pos: current, tick: current_tick, .. }) = open_set.pop() {
            if current == goal {
                return Some(reconstruct_path(&came_from, current));
            }

            if closed_set.contains(&current) {
                continue;
            }
            closed_set.insert(current);

            expansions += 1;
            if expansions > MAX_EXPANSIONS {
                return None;
            }

            let current_g_score = *g_score.get(&current).unwrap_or(&0);

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

                let step_cost = cost_fn(&neighbor, goal, next_tick);
                let tentative_g = current_g_score + step_cost;

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    tick_map.insert(neighbor, next_tick);
                    open_set.push(Node {
                        pos: neighbor,
                        f_score: tentative_g + heuristic(neighbor, goal),
                        tick: next_tick,
                    });
                }
            }
        }

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
