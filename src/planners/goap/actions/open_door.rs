use crate::infra::{Color, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct OpenDoorAction {
    pub color: Color,
    pub door_pos: Position,
}

impl GOAPActionTrait for OpenDoorAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        let has_key = matches!(
            (self.color, &player.inventory),
            (Color::Red, Inventory::KeyRed)
                | (Color::Green, Inventory::KeyGreen)
                | (Color::Blue, Inventory::KeyBlue)
        );
        let door_exists = world
            .doors
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        has_key
            && door_exists
            && world
                .find_path_for_player(player_index, player.position, self.door_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        state.world.players[player_index].inventory = Inventory::None;
        //state.world.players[player_index].position = self.door_pos;
        // Remove door from map (for planning simulation)
        state
            .world
            .map
            .insert(self.door_pos, crate::swoq_interface::Tile::Empty);
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_use_adjacent(world, player_index, self.door_pos, execution_state)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        10.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to door + 1 tick to open it
        state
            .world
            .path_distance(state.world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as u32
            + 1
    }

    fn reward(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // High reward for opening doors (unlocks new areas)
        20.0
    }

    fn name(&self) -> &'static str {
        "OpenDoor"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        // Generate actions for all doors of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(door_positions) = world.doors.get_positions(color) {
                for door_pos in door_positions {
                    let action = OpenDoorAction {
                        color,
                        door_pos: *door_pos,
                    };
                    // Only include if precondition is satisfied
                    if action.precondition(state, player_index) {
                        actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
