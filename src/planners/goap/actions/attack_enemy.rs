use crate::infra::Position;
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct AttackEnemyAction {
    pub enemy_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for AttackEnemyAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        // Path reachability validated during generation
        // Need sword and health > 5 to attack (enemy will hit back for 5 damage)
        player.has_sword && player.health > 5 && world.enemies.get_positions().contains(&self.enemy_pos)
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        // Remove enemy from map (for planning simulation) by replacing with empty tile
        state
            .world
            .map
            .insert(self.enemy_pos, crate::swoq_interface::Tile::Empty);
        let player = &mut state.world.players[player_index];
        player.position = self.enemy_pos;
        // Enemy hits back - lose 5 health
        player.health -= 5;
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_use_adjacent(world, player_index, self.enemy_pos, execution_state)
    }

    fn cost(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        15.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &PlannerState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to attack
    }

    fn name(&self) -> &'static str {
        "AttackEnemy"
    }

    fn reward(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // Positive reward for attacking nearby enemies when armed
        15.0
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate attack actions if player has a sword and health > 5
        if !player.has_sword || player.health <= 5 {
            return actions;
        }

        for enemy_pos in world.enemies.get_positions() {
            let dist = world.path_distance_to_enemy(player.position, *enemy_pos);
            
            // Only generate attack action if enemy is close (within 3 tiles)
            if dist <= 3
                && let Some(path) = world.find_path_for_player(player_index, player.position, *enemy_pos)
            {
                let action = AttackEnemyAction {
                    enemy_pos: *enemy_pos,
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
