//! HuntEnemy action - actively seek out enemies when maze is explored

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct HuntEnemyAction {}

impl RLActionTrait for HuntEnemyAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Only hunt when maze is fully explored
        if !player.unexplored_frontier.is_empty() {
            return false;
        }

        // Need sword and health > 7 to hunt enemies
        if !player.has_sword || player.health < 7 {
            return false;
        }

        // Don't hunt if closest enemy is already in attack range (≤3 tiles)
        if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            let dist = world.path_distance_to_enemy(player.position, closest_enemy);
            if dist <= 3 {
                return false;
            }
        }

        // Only hunt enemies if all doors cannot be opened with discovered items
        if world.can_any_door_be_opened() {
            return false;
        }

        true
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];

        if let Some(current_dest) = player.current_destination
            && player.position != current_dest
        {
            return Some(current_dest);
        }

        // Priority 1: Known enemies
        if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            return Some(closest_enemy);
        }

        // Priority 2: Potential enemy locations
        if let Some(closest_potential) = world
            .potential_enemy_locations
            .iter()
            .min_by_key(|pos| player.position.distance(pos))
            .copied()
        {
            return Some(closest_potential);
        }

        // Priority 3: Random walkable location
        use rand::Rng;
        let mut rng = rand::rng();
        for _ in 0..100 {
            let random_x = rng.random_range(0..world.map.width);
            let random_y = rng.random_range(0..world.map.height);
            let random_pos = Position::new(random_x, random_y);

            if world.is_walkable(&random_pos, None) {
                return Some(random_pos);
            }
        }

        None
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        if !player.has_sword || player.health <= 2 {
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        let Some(target) = player.current_destination else {
            return (DirectedAction::None, ExecutionStatus::Complete);
        };

        // Check if enemy is in attack range
        let enemy_in_range = world
            .enemies
            .closest_to(player.position)
            .map(|pos| world.path_distance_to_enemy(player.position, pos) <= 3)
            .unwrap_or(false);

        if enemy_in_range {
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        execute_move_to(world, player_index, target, execution_state)
    }

    fn name(&self) -> String {
        "HuntEnemy".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::HuntEnemy as usize
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        // Only generate when fully explored
        if !player.unexplored_frontier.is_empty() {
            return actions;
        }

        // Only generate hunt actions if player has a sword and health > 6
        if !player.has_sword || player.health <= 6 {
            return actions;
        }

        let action = HuntEnemyAction {};
        if action.precondition(world, player_index) {
            actions.push(Box::new(action) as Box<dyn RLActionTrait>);
        }

        actions
    }
}
