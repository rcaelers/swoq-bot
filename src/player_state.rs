use std::collections::HashSet;

use crate::map::Map;
use crate::swoq_interface::{Inventory, Tile};
use crate::types::Position;

pub trait Player {
    fn position(&self) -> Position;
    fn health(&self) -> i32;
    fn inventory(&self) -> Inventory;
    fn has_sword(&self) -> bool;
    fn previous_goal(&self) -> Option<crate::goal::Goal>;
    fn current_destination(&self) -> Option<Position>;
    fn current_path(&self) -> Option<Vec<Position>>;
    fn unexplored_frontier(&self) -> &HashSet<Position>;

    fn set_current_destination(&mut self, dest: Option<Position>);
    fn set_current_path(&mut self, path: Option<Vec<Position>>);
    fn set_previous_goal(&mut self, goal: Option<crate::goal::Goal>);

    fn update_frontier(&mut self, map: &Map);
    fn sorted_unexplored(&self) -> Vec<Position>;
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub position: Position,
    pub health: i32,
    pub inventory: Inventory,
    pub has_sword: bool,
    // Planning state to avoid oscillation
    pub previous_goal: Option<crate::goal::Goal>,
    pub current_destination: Option<Position>,
    pub current_path: Option<Vec<Position>>,
    pub unexplored_frontier: HashSet<Position>,
}

impl PlayerState {
    pub fn new(pos: Position) -> Self {
        Self {
            position: pos,
            health: 10,
            inventory: Inventory::None,
            has_sword: false,
            previous_goal: None,
            current_destination: None,
            current_path: None,
            unexplored_frontier: HashSet::new(),
        }
    }

    pub fn clear(&mut self) {
        self.inventory = Inventory::None;
        self.has_sword = false;
        self.previous_goal = None;
        self.current_destination = None;
        self.current_path = None;
        self.unexplored_frontier.clear();
    }

    #[tracing::instrument(level = "trace", skip(self, map))]
    pub fn update_frontier(&mut self, map: &Map) {
        let mut frontier =
            crate::pathfinding::AStar::compute_reachable_positions(map, self.position, |pos| {
                // Optimistic walkability: treat Unknown and None as walkable
                match map.get(pos) {
                    Some(Tile::Wall) | Some(Tile::Boulder) | Some(Tile::Enemy) => false,
                    // Doors without keys are barriers - check player inventory only
                    Some(Tile::DoorRed) => matches!(self.inventory, Inventory::KeyRed),
                    Some(Tile::DoorGreen) => matches!(self.inventory, Inventory::KeyGreen),
                    Some(Tile::DoorBlue) => matches!(self.inventory, Inventory::KeyBlue),
                    // Unknown and None are optimistically walkable
                    _ => true,
                }
            });

        // Filter to only keep positions that are actually Unknown or None
        // This ensures the frontier only contains unexplored tiles
        frontier.retain(|pos| matches!(map.get(pos), Some(Tile::Unknown) | None));

        self.unexplored_frontier = frontier;
        tracing::trace!(frontier_size = self.unexplored_frontier.len(), "Frontier updated");
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sorted_unexplored(&self) -> Vec<Position> {
        let mut frontier: Vec<Position> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.position.distance(pos));
        frontier
    }
}

impl Player for PlayerState {
    fn position(&self) -> Position {
        self.position
    }

    fn health(&self) -> i32 {
        self.health
    }

    fn inventory(&self) -> Inventory {
        self.inventory
    }

    fn has_sword(&self) -> bool {
        self.has_sword
    }

    fn previous_goal(&self) -> Option<crate::goal::Goal> {
        self.previous_goal.clone()
    }

    fn current_destination(&self) -> Option<Position> {
        self.current_destination
    }

    fn current_path(&self) -> Option<Vec<Position>> {
        self.current_path.clone()
    }

    fn unexplored_frontier(&self) -> &HashSet<Position> {
        &self.unexplored_frontier
    }

    fn set_current_destination(&mut self, dest: Option<Position>) {
        self.current_destination = dest;
    }

    fn set_current_path(&mut self, path: Option<Vec<Position>>) {
        self.current_path = path;
    }

    fn set_previous_goal(&mut self, goal: Option<crate::goal::Goal>) {
        self.previous_goal = goal;
    }

    fn update_frontier(&mut self, map: &Map) {
        self.update_frontier(map);
    }

    fn sorted_unexplored(&self) -> Vec<Position> {
        self.sorted_unexplored()
    }
}

impl Player for &mut PlayerState {
    fn position(&self) -> Position {
        self.position
    }

    fn health(&self) -> i32 {
        self.health
    }

    fn inventory(&self) -> Inventory {
        self.inventory
    }

    fn has_sword(&self) -> bool {
        self.has_sword
    }

    fn previous_goal(&self) -> Option<crate::goal::Goal> {
        self.previous_goal.clone()
    }

    fn current_destination(&self) -> Option<Position> {
        self.current_destination
    }

    fn current_path(&self) -> Option<Vec<Position>> {
        self.current_path.clone()
    }

    fn unexplored_frontier(&self) -> &HashSet<Position> {
        &self.unexplored_frontier
    }

    fn set_current_destination(&mut self, dest: Option<Position>) {
        self.current_destination = dest;
    }

    fn set_current_path(&mut self, path: Option<Vec<Position>>) {
        self.current_path = path;
    }

    fn set_previous_goal(&mut self, goal: Option<crate::goal::Goal>) {
        self.previous_goal = goal;
    }

    fn update_frontier(&mut self, map: &Map) {
        PlayerState::update_frontier(self, map);
    }

    fn sorted_unexplored(&self) -> Vec<Position> {
        PlayerState::sorted_unexplored(self)
    }
}
