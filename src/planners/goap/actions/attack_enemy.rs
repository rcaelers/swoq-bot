use crate::infra::Position;
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct AttackEnemyAction {
    pub enemy_pos: Position,
}

impl GOAPActionTrait for AttackEnemyAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        player.has_sword
            && world.enemies.get_positions().contains(&self.enemy_pos)
            && world
                .find_path_for_player(player_index, player.position, self.enemy_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        // Remove enemy from map (for planning simulation) by replacing with empty tile
        state
            .world
            .map
            .insert(self.enemy_pos, crate::swoq_interface::Tile::Empty);
        state.world.players[player_index].position = self.enemy_pos;
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_use_adjacent(world, player_index, self.enemy_pos, execution_state)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        15.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.enemy_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to enemy + 1 tick to attack
        state
            .world
            .path_distance(state.world.players[player_index].position, self.enemy_pos)
            .unwrap_or(1000) as u32
            + 1
    }

    fn reward(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // High reward for clearing enemies (required for exit in most levels)
        25.0
    }

    fn name(&self) -> &'static str {
        "AttackEnemy"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        for enemy_pos in world.enemies.get_positions() {
            let action = AttackEnemyAction {
                enemy_pos: *enemy_pos,
            };
            if action.precondition(state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
