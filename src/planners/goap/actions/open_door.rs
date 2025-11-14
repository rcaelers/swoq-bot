use crate::infra::{Color, Position};
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct OpenDoorAction {
    pub color: Color,
    pub door_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for OpenDoorAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
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
        // Path reachability validated during generation
        has_key && door_exists
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
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
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_use_adjacent(world, player_index, self.door_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to open it
    }

    fn name(&self) -> &'static str {
        "OpenDoor"
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Generate actions for all doors of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(door_positions) = world.doors.get_positions(color) {
                for door_pos in door_positions {
                    if let Some(path) = world.find_path_for_player(player_index, player.position, *door_pos) {
                        let action = OpenDoorAction {
                            color,
                            door_pos: *door_pos,
                            cached_distance: path.len() as u32,
                        };
                        if action.precondition(state, player_index) {
                            actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                        }
                    }
                }
            }
        }

        actions
    }
}
