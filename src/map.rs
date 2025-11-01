use std::collections::HashMap;

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

    /// Check if there are any boulders visible on the map
    pub fn has_boulders(&self) -> bool {
        self.tiles
            .values()
            .any(|tile| matches!(tile, Tile::Boulder))
    }
}
