//! PickupSword action - pick up a sword

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct PickupSwordAction {
    pub sword_pos: Position,
    pub cached_distance: u32,
}

impl RLActionTrait for PickupSwordAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must not already have a sword
        if player.has_sword {
            return false;
        }

        // Sword must exist and be reachable
        world.swords.get_positions().contains(&self.sword_pos)
            && world.find_path(player.position, self.sword_pos).is_some()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        if world.find_path(player.position, self.sword_pos).is_some() {
            Some(self.sword_pos)
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
        execute_move_to(world, player_index, self.sword_pos, execution_state)
    }

    fn name(&self) -> String {
        "PickupSword".to_string()
    }

    fn action_type_index(&self) -> usize {
        ActionType::PickupSword as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.sword_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        for sword_pos in world.swords.get_positions() {
            let cached_distance = world
                .find_path(player.position, *sword_pos)
                .map(|p| p.len() as u32)
                .unwrap_or(0);

            let action = PickupSwordAction {
                sword_pos: *sword_pos,
                cached_distance,
            };
            if action.precondition(world, player_index) {
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        }

        actions
    }
}
