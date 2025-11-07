use std::collections::HashMap;

use crate::infra::Position;
use crate::swoq_interface::Tile;

#[derive(Clone, Debug)]
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
}
