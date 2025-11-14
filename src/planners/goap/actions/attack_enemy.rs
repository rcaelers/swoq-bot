use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct AttackEnemyAction {}

impl GOAPActionTrait for AttackEnemyAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        // Need sword, health >= 7 to attack, and enemies must exist
        player.has_sword && player.health >= 7 && !world.enemies.is_empty()
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
        let player_pos = state.world.players[player_index].position;

        // Find closest enemy and attack it
        if let Some(closest_enemy) = state.world.enemies.closest_to(player_pos) {
            // Remove enemy from map (for planning simulation)
            state
                .world
                .map
                .insert(closest_enemy, crate::swoq_interface::Tile::Empty);

            // Remove enemy from tracking
            state.world.enemies.remove(closest_enemy);

            // Move player to enemy position
            state.world.players[player_index].position = closest_enemy;
            // Enemy hits back - lose 6 health
            state.world.players[player_index].health -= 6;
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        // Check if we should stop attacking: health too low
        if player.health < 2 {
            // Health too low to continue attacking (would die)
            execution_state.enemy_under_attack = None;
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        // Find closest enemy in current world state (enemies move!)
        if let Some(closest_enemy_pos) = world.enemies.closest_to(player.position) {
            let (action, status) =
                execute_use_adjacent(world, player_index, closest_enemy_pos, execution_state);

            // If we just performed a 'use' action (attacked), store the enemy position
            if matches!(status, ExecutionStatus::Complete)
                && !matches!(action, DirectedAction::None)
            {
                if execution_state.enemy_under_attack.is_none() {
                    // First attack - store enemy position
                    execution_state.enemy_under_attack = Some(closest_enemy_pos);
                    (action, ExecutionStatus::InProgress)
                } else {
                    // Subsequent attacks - check if enemy still exists at stored position
                    let enemy_still_alive = world
                        .enemies
                        .get_positions()
                        .iter()
                        .any(|pos| Some(*pos) == execution_state.enemy_under_attack);

                    if enemy_still_alive {
                        // Enemy still alive, continue attacking
                        (action, ExecutionStatus::InProgress)
                    } else {
                        // Enemy is dead (not at stored position anymore)
                        execution_state.enemy_under_attack = None;
                        (action, ExecutionStatus::Complete)
                    }
                }
            } else {
                // Still moving towards enemy
                (action, status)
            }
        } else {
            // No enemies - action complete
            execution_state.enemy_under_attack = None;
            (DirectedAction::None, ExecutionStatus::Complete)
        }
    }

    fn cost(&self, state: &GameState, player_index: usize) -> f32 {
        let world = &state.world;
        let player = &world.players[player_index];

        let distance = if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            world
                .find_path_for_player(player_index, player.position, closest_enemy)
                .map(|p| (p.len() as u32).saturating_sub(1))
                .unwrap_or(100)
        } else {
            100
        };

        15.0 + distance as f32 * 0.1
    }

    fn duration(&self, state: &GameState, player_index: usize) -> u32 {
        let world = &state.world;
        let player = &world.players[player_index];

        if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            world
                .find_path_for_player(player_index, player.position, closest_enemy)
                .map(|p| p.len() as u32)
                .unwrap_or(100)
        } else {
            100
        }
    }

    fn name(&self) -> &'static str {
        "AttackEnemy"
    }

    fn reward(&self, _state: &GameState, _player_index: usize) -> f32 {
        // Positive reward for attacking nearby enemies when armed
        15.0
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate attack actions if player has a sword and health >= 7
        if !player.has_sword || player.health < 7 {
            return actions;
        }

        // When maze is fully explored, attack enemies at any distance
        // Otherwise, only attack close enemies (within 3 tiles)
        let max_distance = if player.unexplored_frontier.is_empty() {
            i32::MAX // No limit when fully explored
        } else {
            3 // Only nearby enemies while exploring
        };

        // Check if any enemy is within range and log distances
        let mut closest_dist = i32::MAX;
        for enemy_pos in world.enemies.get_positions() {
            let dist = world.path_distance_to_enemy(player.position, *enemy_pos);
            tracing::debug!(
                "AttackEnemy::generate - Player {} enemy at {:?}, distance={}",
                player_index,
                enemy_pos,
                dist
            );
            if dist < closest_dist {
                closest_dist = dist;
            }
        }

        let has_enemy_in_range = closest_dist <= max_distance;

        if has_enemy_in_range {
            let action = AttackEnemyAction {};
            if action.precondition(state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
