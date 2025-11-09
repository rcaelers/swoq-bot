use crate::infra::{Color, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct WaitOnPlateAction {
    pub color: Color,
    pub plate_pos: Position,
}

impl GOAPActionTrait for WaitOnPlateAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        world
            .pressure_plates
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.plate_pos))
            && world
                .find_path_for_player(player_index, player.position, self.plate_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        state.world.players[player_index].position = self.plate_pos;
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];
        if player.position == self.plate_pos {
            (DirectedAction::None, ExecutionStatus::InProgress)
        } else {
            execute_move_to(world, player_index, self.plate_pos, execution_state)
        }
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        5.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.plate_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to plate + time waiting (for synchronized multi-player actions)
        // We estimate the synchronized partner will need this long
        let distance = state
            .world
            .path_distance(state.world.players[player_index].position, self.plate_pos)
            .unwrap_or(1000) as u32;
        distance + 5 // +5 ticks for partner to complete their part
    }

    fn name(&self) -> &'static str {
        "WaitOnPlate"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        for color in [
            crate::infra::Color::Red,
            crate::infra::Color::Green,
            crate::infra::Color::Blue,
        ] {
            if let Some(plate_positions) = world.pressure_plates.get_positions(color) {
                for plate_pos in plate_positions {
                    let action = WaitOnPlateAction {
                        color,
                        plate_pos: *plate_pos,
                    };
                    if action.precondition(state, player_index) {
                        actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
