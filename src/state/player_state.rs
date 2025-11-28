use std::collections::HashSet;

use crate::swoq_interface::Inventory;
use crate::infra::Position;

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub position: Position,
    pub health: i32,
    pub inventory: Inventory,
    pub has_sword: bool,
    pub is_active: bool,
    pub current_destination: Option<Position>,
    pub current_path: Option<Vec<Position>>,
    pub unexplored_frontier: HashSet<Position>,
    /// For coop door coordination: the target position this player is trying to reach
    /// Set by PassThroughDoorWithPlateAction, read by WaitOnPlateAction on other player
    pub coop_door_target: Option<Position>,
}

impl PlayerState {
    pub fn new(pos: Position) -> Self {
        Self {
            position: pos,
            health: 5,
            inventory: Inventory::None,
            has_sword: false,
            is_active: true,
            current_destination: None,
            current_path: None,
            unexplored_frontier: HashSet::new(),
            coop_door_target: None,
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sorted_unexplored(&self) -> Vec<Position> {
        let mut frontier: Vec<Position> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.position.distance(pos));
        frontier
    }
}
