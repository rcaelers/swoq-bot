use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_avoid;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct AvoidEnemyAction {}

impl GOAPActionTrait for AvoidEnemyAction {
    fn precondition(&self, state: &GameState, _player_index: usize) -> bool {
        !state.world.enemies.is_empty()
    }

    fn effect(&self, _state: &mut GameState, _player_index: usize) {}

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        // Find closest enemy in current world state (enemies move!)
        if let Some(closest_enemy_pos) = world.enemies.closest_to(player.position) {
            let distance = world.path_distance_to_enemy(player.position, closest_enemy_pos);
            
            // Continue avoiding until enemy is at least 3 tiles away
            if distance >= 3 {
                return (DirectedAction::None, ExecutionStatus::Complete);
            }
            
            let (action, _status) = execute_avoid(world, player_index, closest_enemy_pos);
            (action, ExecutionStatus::InProgress)
        } else {
            // No enemies - action complete
            (DirectedAction::None, ExecutionStatus::Complete)
        }
    }

    fn cost(&self, state: &GameState, player_index: usize) -> f32 {
        let player = &state.world.players[player_index];
        let distance = if let Some(closest_enemy) = state.world.enemies.closest_to(player.position)
        {
            state
                .world
                .path_distance_to_enemy(player.position, closest_enemy)
        } else {
            1000
        };
        5.0 + distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        // Avoidance is immediate, takes 1 tick to move away
        1
    }

    fn name(&self) -> &'static str {
        "AvoidEnemy"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn reward(&self, _state: &GameState, _player_index: usize) -> f32 {
        // Positive reward for avoiding enemies when vulnerable
        2000.0
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate avoid actions if player doesn't have a sword
        if player.has_sword {
            return actions;
        }

        // Check if any enemy is close (within 3 tiles)
        let has_close_enemy = world.enemies.get_positions().iter().any(|enemy_pos| {
            let dist = world.path_distance_to_enemy(player.position, *enemy_pos);
            dist < 3
        });

        if has_close_enemy {
            let action = AvoidEnemyAction {};
            if action.precondition(state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
