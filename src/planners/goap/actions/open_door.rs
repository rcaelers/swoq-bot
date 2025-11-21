use crate::infra::{Color, Position};
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
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

impl OpenDoorAction {
    fn check_execute_precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];
        player
            .position
            .neighbors()
            .iter()
            .any(|adj| *adj == self.door_pos || world.find_path(player.position, *adj).is_some())
    }
}

impl GOAPActionTrait for OpenDoorAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // For planning: player must have the matching key
        let has_key = matches!(
            (self.color, &player.inventory),
            (Color::Red, Inventory::KeyRed)
                | (Color::Green, Inventory::KeyGreen)
                | (Color::Blue, Inventory::KeyBlue)
        );
        if !has_key {
            return false;
        }

        // Door must exist
        let door_exists = world
            .doors
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        if !door_exists {
            return false;
        }

        // Validate path exists to adjacent position
        if !player
            .position
            .neighbors()
            .iter()
            .any(|adj| *adj == self.door_pos || world.find_path(player.position, *adj).is_some())
        {
            return false;
        }

        // Check if this resource is already claimed by another player
        let claim = ResourceClaim::Door(self.color);
        let already_claimed = state
            .resource_claims
            .get(&claim)
            .is_some_and(|&claimer| claimer != player_index);

        !already_claimed
    }

    fn effect_start(
        &self,
        _world: &mut WorldState,
        state: &mut PlanningState,
        player_index: usize,
    ) {
        // Claim this door to prevent other players from targeting it
        let claim = ResourceClaim::Door(self.color);
        state.resource_claims.insert(claim, player_index);
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].inventory = Inventory::None;
        world.players[player_index].position = self.door_pos;
        // Remove door from map (for planning simulation)
        world
            .map
            .insert(self.door_pos, crate::swoq_interface::Tile::Empty);
        world.doors.remove(self.color, self.door_pos);
    }

    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        Some(self.door_pos)
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        // Check precondition before executing
        if !self.check_execute_precondition(world, player_index) {
            return (DirectedAction::None, ExecutionStatus::Wait);
        }

        execute_use_adjacent(world, player_index, self.door_pos, execution_state)
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to open it
    }

    fn name(&self) -> String {
        format!("OpenDoor({:?})", self.color)
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &world;
        let player = &world.players[player_index];

        // Generate actions for all doors of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(door_positions) = world.doors.get_positions(color) {
                for door_pos in door_positions {
                    if let Some(path) = world.find_path(player.position, *door_pos) {
                        let action = OpenDoorAction {
                            color,
                            door_pos: *door_pos,
                            cached_distance: path.len() as u32,
                        };
                        if action.precondition(world, state, player_index) {
                            actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                        }
                    }
                }
            }
        }

        actions
    }
}
