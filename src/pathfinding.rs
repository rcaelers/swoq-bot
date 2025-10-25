use crate::world_state::{Pos, WorldState};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

#[derive(Clone, Eq, PartialEq)]
struct Node {
    pos: Pos,
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
    #[tracing::instrument(level = "trace", skip(world), fields(start_x = start.x, start_y = start.y, goal_x = goal.x, goal_y = goal.y))]
    pub fn find_path(
        world: &WorldState,
        start: Pos,
        goal: Pos,
        can_open_doors: bool,
    ) -> Option<Vec<Pos>> {
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<Pos, Pos> = HashMap::new();
        let mut g_score: HashMap<Pos, i32> = HashMap::new();
        let mut closed_set: HashSet<Pos> = HashSet::new();

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
                    || neighbor.x >= world.map_width
                    || neighbor.y < 0
                    || neighbor.y >= world.map_height
                {
                    continue;
                }

                // Pass the goal to is_walkable so we can walk on the destination key
                if !world.is_walkable_with_goal(&neighbor, goal) {
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

    /// Compute all reachable positions from start using optimistic assumptions
    /// (treating Unknown/None as walkable). Returns a HashSet of reachable frontier positions
    /// (positions that are Unknown or None and adjacent to explored/known tiles).
    /// This combines reachability checking with frontier detection in a single pass.
    #[tracing::instrument(level = "trace", skip(world), fields(start_x = start.x, start_y = start.y))]
    pub fn compute_reachable_positions(world: &WorldState, start: Pos) -> HashSet<Pos> {
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
                    || neighbor.x >= world.map_width
                    || neighbor.y < 0
                    || neighbor.y >= world.map_height
                {
                    continue;
                }

                // Check if this is an unexplored tile (frontier candidate)
                let is_unexplored = matches!(
                    world.map.get(&neighbor),
                    Some(crate::swoq_interface::Tile::Unknown) | None
                );

                // Optimistic walkability: treat Unknown and None as walkable
                let walkable = match world.map.get(&neighbor) {
                    Some(crate::swoq_interface::Tile::Wall)
                    | Some(crate::swoq_interface::Tile::Boulder)
                    | Some(crate::swoq_interface::Tile::Enemy) => false,
                    // Doors without keys are barriers
                    Some(crate::swoq_interface::Tile::DoorRed) => {
                        world.has_key(crate::world_state::Color::Red)
                    }
                    Some(crate::swoq_interface::Tile::DoorGreen) => {
                        world.has_key(crate::world_state::Color::Green)
                    }
                    Some(crate::swoq_interface::Tile::DoorBlue) => {
                        world.has_key(crate::world_state::Color::Blue)
                    }
                    // Unknown and None are optimistically walkable
                    _ => true,
                };

                if walkable {
                    reachable.insert(neighbor);
                    queue.push_back(neighbor);

                    // If this is unexplored and we reached it from an explored tile,
                    // it's part of the frontier
                    if is_unexplored {
                        // Check if current position is explored (not Unknown/None)
                        let current_is_explored = match world.map.get(&current) {
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

fn heuristic(a: Pos, b: Pos) -> i32 {
    a.distance(&b)
}

fn reconstruct_path(came_from: &HashMap<Pos, Pos>, mut current: Pos) -> Vec<Pos> {
    let mut path = vec![current];
    while let Some(&prev) = came_from.get(&current) {
        path.push(prev);
        current = prev;
    }
    path.reverse();
    path
}
