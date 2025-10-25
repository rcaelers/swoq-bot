use crate::swoq_interface::Tile;
use crate::world_state::Pos;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boulder {
    pub pos: Pos,
    pub has_moved: bool,
}

impl Boulder {
    pub fn new(pos: Pos, has_moved: bool) -> Self {
        Self { pos, has_moved }
    }
}

#[derive(Debug, Clone)]
pub struct BoulderTracker {
    boulders: HashMap<Pos, Boulder>,
}

impl BoulderTracker {
    pub fn new() -> Self {
        Self {
            boulders: HashMap::new(),
        }
    }

    pub fn add_boulder(&mut self, pos: Pos, has_moved: bool) {
        self.boulders.insert(pos, Boulder::new(pos, has_moved));
    }

    pub fn remove_boulder(&mut self, pos: &Pos) -> Option<Boulder> {
        self.boulders.remove(pos)
    }

    pub fn get_all_positions(&self) -> Vec<Pos> {
        self.boulders.keys().copied().collect()
    }

    pub fn get_original_boulders(&self) -> Vec<Pos> {
        self.boulders
            .values()
            .filter(|b| !b.has_moved)
            .map(|b| b.pos)
            .collect()
    }

    pub fn contains(&self, pos: &Pos) -> bool {
        self.boulders.contains_key(pos)
    }

    pub fn has_moved(&self, pos: &Pos) -> bool {
        self.boulders.get(pos).map(|b| b.has_moved).unwrap_or(false)
    }

    pub fn clear(&mut self) {
        self.boulders.clear();
    }

    pub fn len(&self) -> usize {
        self.boulders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boulders.is_empty()
    }

    /// Update boulder positions based on newly seen boulders and current map state
    pub fn update<F>(&mut self, seen_boulders: Vec<Pos>, map: &HashMap<Pos, Tile>, is_adjacent: F)
    where
        F: Fn(&Pos) -> bool,
    {
        // Add newly seen boulders
        for boulder_pos in seen_boulders {
            if !self.contains(&boulder_pos) {
                // New boulder discovered - assume it hasn't moved unless it's adjacent (we just dropped it)
                let has_moved = is_adjacent(&boulder_pos);
                self.add_boulder(boulder_pos, has_moved);
            }
        }

        // Remove boulders that have been picked up (turned to Empty or other non-boulder tiles)
        let all_boulder_positions = self.get_all_positions();
        for pos in all_boulder_positions {
            if let Some(tile) = map.get(&pos)
                && !matches!(tile, Tile::Boulder)
            {
                debug!("Boulder at {:?} was picked up or destroyed", pos);
                self.remove_boulder(&pos);
            }
        }
    }
}
