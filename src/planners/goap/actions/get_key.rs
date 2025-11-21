use crate::infra::{Color, Position};
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct GetKeyAction {
    pub color: Color,
    pub key_pos: Position,
    pub cached_distance: u32,
}

impl GetKeyAction {
    fn check_execute_precondition(&self, world: &WorldState, player_index: usize) -> bool {
        // Check if key is reachable (might be blocked until other player opens a door)
        world
            .find_path(world.players[player_index].position, self.key_pos)
            .is_some()
    }
}

impl GOAPActionTrait for GetKeyAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // For planning: player must have empty inventory
        if player.inventory != Inventory::None {
            return false;
        }

        // Key must exist and be reachable
        if !world
            .keys
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.key_pos))
            || world.find_path(player.position, self.key_pos).is_none()
        {
            return false;
        }

        // Check if this resource is already claimed by another player
        let claim = ResourceClaim::Key(self.color);
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
        // Claim this key to prevent other players from targeting it
        let claim = ResourceClaim::Key(self.color);
        state.resource_claims.insert(claim, player_index);
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].inventory = match self.color {
            Color::Red => Inventory::KeyRed,
            Color::Green => Inventory::KeyGreen,
            Color::Blue => Inventory::KeyBlue,
        };
        world.players[player_index].position = self.key_pos;
        // Remove key from map (for planning simulation)
        world
            .map
            .insert(self.key_pos, crate::swoq_interface::Tile::Empty);
        // Remove key from world.keys
        world.keys.remove(self.color, self.key_pos);
    }

    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        Some(self.key_pos)
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

        execute_move_to(world, player_index, self.key_pos, execution_state)
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> String {
        format!("GetKey({:?})", self.color)
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &world;
        let player = &world.players[player_index];

        // Generate actions for all known keys of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(key_positions) = world.keys.get_positions(color) {
                for key_pos in key_positions {
                    // Check path and cache distance
                    if let Some(path) = world.find_path(player.position, *key_pos) {
                        let action = GetKeyAction {
                            color,
                            key_pos: *key_pos,
                            cached_distance: path.len() as u32,
                        };
                        // Check other preconditions
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
