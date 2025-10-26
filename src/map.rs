use std::collections::HashMap;

use crate::pathfinding::AStar;
use crate::swoq_interface::Tile;
use crate::types::Position;

#[derive(Clone)]
pub struct Map {
    pub width: i32,
    pub height: i32,
    tiles: HashMap<Position, Tile>,
}

impl Map {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            tiles: HashMap::new(),
        }
    }

    pub fn get(&self, pos: &Position) -> Option<&Tile> {
        self.tiles.get(pos)
    }

    pub fn insert(&mut self, pos: Position, tile: Tile) -> Option<Tile> {
        self.tiles.insert(pos, tile)
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Position, &mut Tile) -> bool,
    {
        self.tiles.retain(f);
    }

    #[allow(dead_code)]
    pub fn tiles(&self) -> &HashMap<Position, Tile> {
        &self.tiles
    }

    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Position, &Tile)> {
        self.tiles.iter()
    }

    pub fn is_walkable(&self, pos: &Position, goal: Position) -> bool {
        match self.get(pos) {
            Some(
                Tile::Empty
                | Tile::Player // TODO: check 2UP conflict
                | Tile::PressurePlateRed
                | Tile::PressurePlateGreen
                | Tile::PressurePlateBlue
                | Tile::Treasure
            ) => true,
            // Keys: always avoid unless it's the destination
            Some(Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue | Tile::Sword | Tile::Health | Tile::Exit  | Tile::Unknown) => {
                // Allow walking on the destination key, avoid all others
                *pos == goal
            }
            None => *pos == goal,
            _ => false,
        }
    }

    pub fn find_path(&self, start: Position, goal: Position) -> Option<Vec<Position>> {
        AStar::find_path(self, start, goal, |pos, goal| self.is_walkable(pos, goal))
    }

    /// Find a path that avoids colliding with another player's planned path.
    /// For each position at step N in our path, we avoid:
    /// 1. The position where the other player is at step N (same tick collision)
    /// 2. The position where the other player was at step N-1 (swap if P2 moves first)
    /// 3. The position where the other player will be at step N+1 (swap if P1 moves first)
    pub fn find_path_avoiding_player(
        &self,
        start: Position,
        goal: Position,
        other_player_path: &[Position],
    ) -> Option<Vec<Position>> {
        AStar::find_path_with_tick(self, start, goal, |pos, goal_pos, tick| {
            // First check basic walkability
            if !self.is_walkable(pos, goal_pos) {
                return false;
            }

            let tick_index = tick as usize;

            // Check if the other player is at this position at this tick (same tick collision)
            if tick_index < other_player_path.len() {
                if *pos == other_player_path[tick_index] {
                    return false; // Would collide at this tick
                }
            } else if let Some(last_pos) = other_player_path.last() {
                // Other player has finished their path, check their final position
                if *pos == *last_pos {
                    return false; // Other player is resting here
                }
            }

            // Check swap collision: P2 moving to where P1 was (P2 moves first)
            // path_player2[tick] cannot be path_player1[tick-1]
            if tick_index > 0
                && tick_index - 1 < other_player_path.len()
                && *pos == other_player_path[tick_index - 1]
            {
                return false; // Would swap with P1
            }

            // Check swap collision: P2 moving to where P1 will be (P1 moves first)
            // path_player2[tick] cannot be path_player1[tick+1]
            if tick_index + 1 < other_player_path.len() && *pos == other_player_path[tick_index + 1]
            {
                return false; // Would swap with P1
            }

            true
        })
    }

    /// Compute all reachable positions from start using a walkability checker.
    /// Returns a HashSet of reachable frontier positions
    /// (positions that are Unknown or None and adjacent to explored/known tiles).
    /// This combines reachability checking with frontier detection in a single pass.
    #[tracing::instrument(level = "trace", skip(self, is_walkable), fields(start_x = start.x, start_y = start.y))]
    pub fn compute_reachable_positions<F>(
        &self,
        start: Position,
        is_walkable: F,
    ) -> std::collections::HashSet<Position>
    where
        F: Fn(&Position) -> bool,
    {
        let mut reachable = std::collections::HashSet::new();
        let mut frontier = std::collections::HashSet::new();
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
                    || neighbor.x >= self.width
                    || neighbor.y < 0
                    || neighbor.y >= self.height
                {
                    continue;
                }

                // Check if this is an unexplored tile (frontier candidate)
                let is_unexplored = matches!(
                    self.get(&neighbor),
                    Some(crate::swoq_interface::Tile::Unknown) | None
                );

                // Use the provided walkability checker
                let walkable = is_walkable(&neighbor);

                if walkable {
                    reachable.insert(neighbor);
                    queue.push_back(neighbor);

                    // If this is unexplored and we reached it from an explored tile,
                    // it's part of the frontier
                    if is_unexplored {
                        // Check if current position is explored (not Unknown/None)
                        let current_is_explored = match self.get(&current) {
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
