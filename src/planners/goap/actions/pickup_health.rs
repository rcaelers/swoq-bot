use crate::infra::Position;
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupHealthAction {
    pub health_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for PickupHealthAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
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
        
        true
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
        // Heal player +5 (no cap)
        let player = &mut state.world.players[player_index];
        player.health += 5;
        player.position = self.health_pos;
        // Remove health from tracker and map (for planning simulation)
        state.world.health.remove(self.health_pos);
        state
            .world
            .map
            .insert(self.health_pos, crate::swoq_interface::Tile::Empty);
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.health_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        5.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> &'static str {
        "PickupHealth"
    }

    fn reward(&self, state: &GameState, player_index: usize) -> f32 {
        let player = &state.world.players[player_index];
        // Higher reward when health is lower
        // At 5 HP: (1.0 - 0.5) * 20.0 = 10.0
        // At 1 HP: (1.0 - 0.1) * 20.0 = 18.0
        let health_ratio = (player.health as f32 / 10.0).min(1.0);
        (1.0 - health_ratio) * 20.0
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        for health_pos in world.health.get_positions() {
            if let Some(path) = world.find_path_for_player(player_index, player.position, *health_pos) {
                let action = PickupHealthAction {
                    health_pos: *health_pos,
                    cached_distance: path.len() as u32,
                };
                if action.precondition(state, player_index) {
                    actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                }
            }
        }

        actions
    }
}
