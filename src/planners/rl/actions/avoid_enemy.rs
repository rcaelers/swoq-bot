//! AvoidEnemy action - flee from nearby enemies when vulnerable

use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_avoid;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct AvoidEnemyAction {}

impl RLActionTrait for AvoidEnemyAction {
    fn precondition(&self, world: &WorldState, _player_index: usize) -> bool {
        !world.enemies.is_empty()
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        if let Some(closest_enemy_pos) = world.enemies.closest_to(player.position) {
            let distance = world.path_distance_to_enemy(player.position, closest_enemy_pos);

            // Continue avoiding until enemy is at least 3 tiles away
            if distance > 3 {
                return (DirectedAction::None, ExecutionStatus::Complete);
            }

            let (action, _status) = execute_avoid(world, player_index, closest_enemy_pos);
            (action, ExecutionStatus::InProgress)
        } else {
            (DirectedAction::None, ExecutionStatus::Complete)
        }
    }

    fn name(&self) -> String {
        "AvoidEnemy".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::AvoidEnemy as usize
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
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
            if action.precondition(world, player_index) {
                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
            }
        }

        actions
    }
}
