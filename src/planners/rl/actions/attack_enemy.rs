//! AttackEnemy action - attack an adjacent enemy

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_use_adjacent;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct AttackEnemyAction {}

impl RLActionTrait for AttackEnemyAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];
        // Need sword, health >= 7 to attack, and enemies must exist
        player.has_sword && player.health >= 7 && !world.enemies.is_empty()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player_pos = world.players[player_index].position;
        world.enemies.closest_to(player_pos)
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
                    execution_state.enemy_under_attack = Some(closest_enemy_pos);
                    (action, ExecutionStatus::InProgress)
                } else {
                    let enemy_still_alive = world
                        .enemies
                        .get_positions()
                        .iter()
                        .any(|pos| Some(*pos) == execution_state.enemy_under_attack);

                    if enemy_still_alive {
                        (action, ExecutionStatus::InProgress)
                    } else {
                        execution_state.enemy_under_attack = None;
                        (action, ExecutionStatus::Complete)
                    }
                }
            } else {
                (action, status)
            }
        } else {
            execution_state.enemy_under_attack = None;
            (DirectedAction::None, ExecutionStatus::Complete)
        }
    }

    fn name(&self) -> String {
        "AttackEnemy".to_string()
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::AttackEnemy as usize
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        // Only generate attack actions if player has a sword and health >= 7
        if !player.has_sword || player.health < 7 {
            return actions;
        }

        // When maze is fully explored, attack enemies at any distance
        // Otherwise, only attack close enemies (within 3 tiles)
        let max_distance = if player.unexplored_frontier.is_empty() {
            i32::MAX
        } else {
            3
        };

        let mut closest_dist = i32::MAX;
        for enemy_pos in world.enemies.get_positions() {
            let dist = world.path_distance_to_enemy(player.position, *enemy_pos);
            if dist < closest_dist {
                closest_dist = dist;
            }
        }

        let has_enemy_in_range = closest_dist <= max_distance;

        if has_enemy_in_range {
            let action = AttackEnemyAction {};
            if action.precondition(world, player_index) {
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        }

        actions
    }
}
