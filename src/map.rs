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
                | Tile::Exit
                | Tile::Player
                | Tile::Sword
                | Tile::Health
                | Tile::PressurePlateRed
                | Tile::PressurePlateGreen
                | Tile::PressurePlateBlue
                | Tile::Treasure
                | Tile::Unknown, // Fog of war - assume walkable
            ) => true,
            // Keys: always avoid unless it's the destination
            Some(Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue) => {
                // Allow walking on the destination key, avoid all others
                *pos == goal
            }
            // Doors: never walkable, must use a key from adjacent tile
            Some(Tile::DoorRed | Tile::DoorGreen | Tile::DoorBlue) => false,
            None => true, // Never seen tiles - assume walkable
            _ => false,
        }
    }

    pub fn find_path(&self, start: Position, goal: Position) -> Option<Vec<Position>> {
        AStar::find_path(self, start, goal, |pos, goal| self.is_walkable(pos, goal))
    }
}
