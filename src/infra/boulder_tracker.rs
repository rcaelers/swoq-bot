use std::collections::HashMap;
use tracing::debug;

use crate::infra::Position;
use crate::state::Map;
use crate::swoq_interface::Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boulder {
    pub pos: Position,
    pub has_moved: bool,
}

impl Boulder {
    pub fn new(pos: Position, has_moved: bool) -> Self {
        Self { pos, has_moved }
    }
}

#[derive(Debug, Clone)]
pub struct BoulderTracker {
    boulders: HashMap<Position, Boulder>,
}

impl BoulderTracker {
    pub fn new() -> Self {
        Self {
            boulders: HashMap::new(),
        }
    }

    pub fn add_boulder(&mut self, pos: Position, has_moved: bool) {
        self.boulders.insert(pos, Boulder::new(pos, has_moved));
    }

    pub fn remove_boulder(&mut self, pos: &Position) -> Option<Boulder> {
        self.boulders.remove(pos)
    }

    pub fn get_all_positions(&self) -> Vec<Position> {
        self.boulders.keys().copied().collect()
    }

    pub fn get_original_boulders(&self) -> Vec<Position> {
        self.boulders
            .values()
            .filter(|b| !b.has_moved)
            .map(|b| b.pos)
            .collect()
    }

    pub fn contains(&self, pos: &Position) -> bool {
        self.boulders.contains_key(pos)
    }

    pub fn has_moved(&self, pos: &Position) -> bool {
        self.boulders.get(pos).map(|b| b.has_moved).unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.boulders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boulders.is_empty()
    }

    /// Update boulder positions based on newly seen boulders and current map state
    #[tracing::instrument(level = "trace", skip(self, map, is_adjacent), fields(seen_count = seen_boulders.len()))]
    pub fn update<F>(&mut self, seen_boulders: Vec<Position>, map: &Map, is_adjacent: F)
    where
        F: Fn(&Position) -> bool,
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
        // But keep boulders that are on pressure plates (tile shows as PressurePlate*, not Boulder)
        let all_boulder_positions = self.get_all_positions();
        for pos in all_boulder_positions {
            if let Some(tile) = map.get(&pos) {
                let keep_boulder = matches!(
                    tile,
                    Tile::Boulder // | Tile::PressurePlateRed
                                  // | Tile::PressurePlateGreen
                                  // | Tile::PressurePlateBlue
                );
                if !keep_boulder {
                    debug!("Boulder at {:?} was picked up or destroyed (tile: {:?})", pos, tile);
                    self.remove_boulder(&pos);
                } else {
                    debug!("Keeping boulder at {:?} (tile: {:?})", pos, tile);
                }
            }
        }
    }
}
