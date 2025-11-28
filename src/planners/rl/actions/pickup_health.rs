//! PickupHealth action - pick up a health item

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct PickupHealthAction {
    pub health_pos: Position,
    pub cached_distance: u32,
}

impl RLActionTrait for PickupHealthAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Health must exist on map
        if !world.health.get_positions().contains(&self.health_pos) {
            return false;
        }

        // In 2-player mode, only allow pickup if this player has <= health than other player
        if world.players.len() == 2 {
            let other_player_index = if player_index == 0 { 1 } else { 0 };
            let other_player = &world.players[other_player_index];
            if player.health > other_player.health {
                return false;
            }
        }

        // Validate path exists
        world.find_path(player.position, self.health_pos).is_some()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        if world.find_path(player.position, self.health_pos).is_some() {
            Some(self.health_pos)
        } else {
            None
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.health_pos, execution_state)
    }

    fn name(&self) -> String {
        "PickupHealth".to_string()
    }

    fn action_type_index(&self) -> usize {
        ActionType::PickupHealth as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.health_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        for health_pos in world.health.get_positions() {
            let cached_distance = world
                .find_path(player.position, *health_pos)
                .map(|p| p.len() as u32)
                .unwrap_or(0);

            let action = PickupHealthAction {
                health_pos: *health_pos,
                cached_distance,
            };
            if action.precondition(world, player_index) {
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        }

        actions
    }
}
