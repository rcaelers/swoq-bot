use crate::world_state::Pos;
use std::collections::HashMap;

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
pub struct BoulderInfo {
    boulders: HashMap<Pos, Boulder>,
}

impl BoulderInfo {
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
}
