use std::collections::HashSet;

use crate::swoq_interface::Inventory;
use crate::types::Position;

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub position: Position,
    pub health: i32,
    pub inventory: Inventory,
    pub has_sword: bool,
    pub is_active: bool,
    // Planning state to avoid oscillation
    pub current_goal: Option<crate::goals::Goal>,
    pub previous_goal: Option<crate::goals::Goal>,
    pub current_destination: Option<Position>,
    pub current_path: Option<Vec<Position>>,
    pub unexplored_frontier: HashSet<Position>,
    // Oscillation recovery - force random exploration for N ticks
    pub force_random_explore_ticks: i32,
}

impl PlayerState {
    pub fn new(pos: Position) -> Self {
        Self {
            position: pos,
            health: 10,
            inventory: Inventory::None,
            has_sword: false,
            is_active: true,
            current_goal: None,
            previous_goal: None,
            current_destination: None,
            current_path: None,
            unexplored_frontier: HashSet::new(),
            force_random_explore_ticks: 0,
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sorted_unexplored(&self) -> Vec<Position> {
        let mut frontier: Vec<Position> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.position.distance(pos));
        frontier
    }
}
