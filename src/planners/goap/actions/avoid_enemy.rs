use crate::infra::Position;
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_avoid;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct AvoidEnemyAction {
    pub enemy_pos: Position,
}

impl GOAPActionTrait for AvoidEnemyAction {
    fn precondition(&self, state: &PlannerState, _player_index: usize) -> bool {
        state
            .world
            .enemies
            .get_positions()
            .contains(&self.enemy_pos)
    }

    fn effect(&self, _state: &mut PlannerState, _player_index: usize) {}

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_avoid(world, player_index, self.enemy_pos)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        5.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.enemy_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, _state: &PlannerState, _player_index: usize) -> u32 {
        // Avoidance is immediate, takes 1 tick to move away
        1
    }

    fn name(&self) -> &'static str {
        "AvoidEnemy"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn reward(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // Positive reward for avoiding enemies when vulnerable
        2000.0
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate avoid actions if player doesn't have a sword
        if player.has_sword {
            return actions;
        }

        for enemy_pos in world.enemies.get_positions() {
            let dist = world.path_distance_to_enemy(player.position, *enemy_pos);

            // Only generate avoid action if enemy is close (within 3 tiles)
            if dist <= 3 {
                let action = AvoidEnemyAction {
                    enemy_pos: *enemy_pos,
                };
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
