//! TouchPlate action - stand on a pressure plate (idle activity)

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct TouchPlateAction {
    pub plate_pos: Position,
    pub plate_color: Color,
    pub cached_distance: u32,
}

impl RLActionTrait for TouchPlateAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        // Only for single-player mode
        if world.is_two_player_mode() {
            return false;
        }

        let player = &world.players[player_index];

        // Empty inventory
        if player.inventory != Inventory::None {
            return false;
        }

        // No unexplored frontier
        if !player.unexplored_frontier.is_empty() {
            return false;
        }

        // Plate must exist and be reachable
        if let Some(positions) = world.pressure_plates.get_positions(self.plate_color) {
            if !positions.contains(&self.plate_pos)
                || world.find_path(player.position, self.plate_pos).is_none()
            {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        if world.find_path(player.position, self.plate_pos).is_some() {
            Some(self.plate_pos)
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
        let player = &world.players[player_index];
        let player_pos = player.position;

        // If we're already on the plate, wait 2 ticks before completing
        if player_pos == self.plate_pos {
            execution_state.wait_ticks += 1;

            if execution_state.wait_ticks >= 2 {
                world.plates_touched.insert(self.plate_color);
                return (DirectedAction::None, ExecutionStatus::Complete);
            } else {
                return (DirectedAction::None, ExecutionStatus::InProgress);
            }
        }

        execute_move_to(world, player_index, self.plate_pos, execution_state)
    }

    fn name(&self) -> String {
        "TouchPlate".to_string()
    }

    fn action_type_index(&self) -> usize {
        ActionType::TouchPlate as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.plate_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        if player.inventory != Inventory::None {
            return actions;
        }

        if !player.unexplored_frontier.is_empty() {
            return actions;
        }

        if !world.boulders.get_all_positions().is_empty() {
            return actions;
        }

        for &color in &[Color::Red, Color::Green, Color::Blue] {
            if let Some(plate_positions) = world.pressure_plates.get_positions(color) {
                for &plate_pos in plate_positions {
                    let cached_distance = world
                        .find_path(player.position, plate_pos)
                        .map(|p| p.len() as u32)
                        .unwrap_or(0);

                    let action = TouchPlateAction {
                        plate_pos,
                        plate_color: color,
                        cached_distance,
                    };

                    if action.precondition(world, player_index) {
                        actions.push(Box::new(action) as Box<dyn RLActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
