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

    /// Compute all reachable positions from start using a walkability checker.
    /// Returns a HashSet of reachable frontier positions
    /// (positions that are Unknown or None and adjacent to explored/known tiles).
    /// This combines reachability checking with frontier detection in a single pass.
    #[tracing::instrument(level = "trace", skip(map, is_walkable), fields(start_x = start.x, start_y = start.y))]
    pub fn compute_reachable_positions<F>(
        map: &Map,
        start: Position,
        is_walkable: F,
    ) -> HashSet<Position>
    where
        F: Fn(&Position) -> bool,
    {
        let mut reachable = HashSet::new();
        let mut frontier = HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        reachable.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            for neighbor in current.neighbors() {
                // Skip if already visited
                if reachable.contains(&neighbor) {
                    continue;
                }

                // Check bounds
                if neighbor.x < 0
                    || neighbor.x >= map.width
                    || neighbor.y < 0
                    || neighbor.y >= map.height
                {
                    continue;
                }

                // Check if this is an unexplored tile (frontier candidate)
                let is_unexplored =
                    matches!(map.get(&neighbor), Some(crate::swoq_interface::Tile::Unknown) | None);

                // Use the provided walkability checker
                let walkable = is_walkable(&neighbor);

                if walkable {
                    reachable.insert(neighbor);
                    queue.push_back(neighbor);

                    // If this is unexplored and we reached it from an explored tile,
                    // it's part of the frontier
                    if is_unexplored {
                        // Check if current position is explored (not Unknown/None)
                        let current_is_explored = match map.get(&current) {
                            Some(crate::swoq_interface::Tile::Unknown) | None => false,
                            Some(_) => true,
                        };

                        if current_is_explored {
                            frontier.insert(neighbor);
                        }
                    }
                }
            }
        }

        tracing::trace!(
            frontier_size = frontier.len(),
            reachable_size = reachable.len(),
            "Frontier computation complete"
        );
        frontier
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
