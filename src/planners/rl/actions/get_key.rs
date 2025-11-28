//! GetKey action - pick up a colored key

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct GetKeyAction {
    pub color: Color,
    pub key_pos: Position,
    pub cached_distance: u32,
}

impl RLActionTrait for GetKeyAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must have empty inventory
        if player.inventory != Inventory::None {
            return false;
        }

        // Key must exist and be reachable
        world
            .keys
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.key_pos))
            && world.find_path(player.position, self.key_pos).is_some()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        if world.find_path(player.position, self.key_pos).is_some() {
            Some(self.key_pos)
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
        execute_move_to(world, player_index, self.key_pos, execution_state)
    }

    fn name(&self) -> String {
        format!("GetKey({:?})", self.color)
    }

    fn action_type_index(&self) -> usize {
        ActionType::GetKey as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.key_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(key_positions) = world.keys.get_positions(color) {
                for key_pos in key_positions {
                    if let Some(path) = world.find_path(player.position, *key_pos) {
                        let action = GetKeyAction {
                            color,
                            key_pos: *key_pos,
                            cached_distance: path.len() as u32,
                        };
                        if action.precondition(world, player_index) {
                            actions.push(Box::new(action) as Box<dyn RLActionTrait>);
                        }
                    }
                }
            }
        }

        actions
    }
}
