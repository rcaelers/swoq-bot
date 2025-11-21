use crate::infra::Position;
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupHealthAction {
    pub health_pos: Position,
    pub cached_distance: u32,
}

impl PickupHealthAction {
    fn check_execute_precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];
        world.find_path(player.position, self.health_pos).is_some()
    }
}

impl GOAPActionTrait for PickupHealthAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
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

        // Validate path exists
        if world.find_path(player.position, self.health_pos).is_none() {
            return false;
        }

        // Check if this resource is already claimed by another player
        let claim = ResourceClaim::Health(self.health_pos);
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
        // Claim this health pickup to prevent other players from targeting it
        let claim = ResourceClaim::Health(self.health_pos);
        state.resource_claims.insert(claim, player_index);
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        // Heal player +5 (no cap)
        let player = &mut world.players[player_index];
        player.health += 5;
        player.position = self.health_pos;
        // Remove health from tracker and map (for planning simulation)
        world.health.remove(self.health_pos);
        world
            .map
            .insert(self.health_pos, crate::swoq_interface::Tile::Empty);
    }

    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        Some(self.health_pos)
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

        execute_move_to(world, player_index, self.health_pos, execution_state)
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        5.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> String {
        "PickupHealth".to_string()
    }

    fn reward(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> f32 {
        let player = &world.players[player_index];
        // Higher reward when health is lower
        // At 5 HP: (1.0 - 0.5) * 20.0 = 10.0
        // At 1 HP: (1.0 - 0.1) * 20.0 = 18.0
        let health_ratio = (player.health as f32 / 10.0).min(1.0);
        (1.0 - health_ratio) * 20.0
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &world;
        let player = &world.players[player_index];

        for health_pos in world.health.get_positions() {
            let cached_distance = world
                .find_path(player.position, *health_pos)
                .map(|p| p.len() as u32)
                .unwrap_or(0);

            let action = PickupHealthAction {
                health_pos: *health_pos,
                cached_distance,
            };
            if action.precondition(world, state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
