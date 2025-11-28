//! OpenDoor action - use a key to open a door

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct OpenDoorAction {
    pub color: Color,
    pub door_pos: Position,
    pub cached_distance: u32,
}

impl RLActionTrait for OpenDoorAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player must have the matching key
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
        player
            .position
            .neighbors()
            .iter()
            .any(|adj| *adj == self.door_pos || world.find_path(player.position, *adj).is_some())
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        let reachable =
            player.position.neighbors().iter().any(|adj| {
                *adj == self.door_pos || world.find_path(player.position, *adj).is_some()
            });

        if reachable { Some(self.door_pos) } else { None }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_use_adjacent(world, player_index, self.door_pos, execution_state)
    }

    fn name(&self) -> String {
        format!("OpenDoor({:?})", self.color)
    }

    fn action_type_index(&self) -> usize {
        ActionType::OpenDoor as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.door_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(door_positions) = world.doors.get_positions(color) {
                for door_pos in door_positions {
                    if let Some(path) = world.find_path(player.position, *door_pos) {
                        let action = OpenDoorAction {
                            color,
                            door_pos: *door_pos,
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
