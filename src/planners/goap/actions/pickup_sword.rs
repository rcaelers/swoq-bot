use crate::infra::Position;
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupSwordAction {
    pub sword_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for PickupSwordAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // For planning: player must not already have a sword
        if player.has_sword {
            return false;
        }

        // Sword must exist and be reachable
        if !world.swords.get_positions().contains(&self.sword_pos)
            || world.find_path(player.position, self.sword_pos).is_none()
        {
            return false;
        }

        // Check if this resource is already claimed by another player
        let claim = ResourceClaim::Sword(self.sword_pos);
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
        // Claim this sword to prevent other players from targeting it
        let claim = ResourceClaim::Sword(self.sword_pos);
        state.resource_claims.insert(claim, player_index);
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].has_sword = true;
        world.players[player_index].position = self.sword_pos;
        // Remove sword from tracker and map (for planning simulation)
        world.swords.remove(self.sword_pos);
        world
            .map
            .insert(self.sword_pos, crate::swoq_interface::Tile::Empty);
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        if world.find_path(player.position, self.sword_pos).is_some() {
            Some(self.sword_pos)
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
        execute_move_to(world, player_index, self.sword_pos, execution_state)
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> String {
        "PickupSword".to_string()
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &world;
        let player = &world.players[player_index];

        for sword_pos in world.swords.get_positions() {
            // Calculate distance for cost/duration, path validation happens in precondition
            let cached_distance = world
                .find_path(player.position, *sword_pos)
                .map(|p| p.len() as u32)
                .unwrap_or(0);

            let action = PickupSwordAction {
                sword_pos: *sword_pos,
                cached_distance,
            };
            if action.precondition(world, state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
