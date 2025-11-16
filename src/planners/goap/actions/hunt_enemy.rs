use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct HuntEnemyAction {
    // No cached target - determined during execution
}

impl GOAPActionTrait for HuntEnemyAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Only hunt when maze is fully explored
        if !player.unexplored_frontier.is_empty() {
            return false;
        }

        // Need sword and health > 7 to hunt enemies (lose 6 per hit, stop at 2)
        if !player.has_sword || player.health < 7 {
            return false;
        }

        // Don't hunt if closest enemy is already in attack range (≤3 tiles)
        if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            let dist = world.path_distance_to_enemy(player.position, closest_enemy);
            if dist <= 3 {
                return false; // AttackEnemy should handle this
            }
        }

        // Only hunt enemies if all doors cannot be opened with discovered items
        // (no point hunting if we still have doors to open for exploration)
        if world.can_any_door_be_opened() {
            return false;
        }
        true
    }

    fn effect_end(&self, _state: &mut GameState, _player_index: usize) {
        // Effect doesn't matter for terminal actions - execution handles everything
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        // Check if we still have sword and health
        if !player.has_sword || player.health <= 2 {
            tracing::debug!(
                "HuntEnemy: Player {} doesn't have sword or health too low ({})",
                player_index,
                player.health
            );
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        // Find closest enemy in current world state (enemies move!)
        if let Some(closest_enemy_pos) = world.enemies.closest_to(player.position) {
            let dist = world.path_distance_to_enemy(player.position, closest_enemy_pos);

            // Stop when enemy is in range (≤3 tiles) - let AttackEnemy handle it
            if dist <= 3 {
                tracing::debug!(
                    "HuntEnemy: Player {} found enemy at {:?} within range ({}), completing",
                    player_index,
                    closest_enemy_pos,
                    dist
                );
                return (DirectedAction::None, ExecutionStatus::Complete);
            }

            tracing::debug!(
                "HuntEnemy: Player {} moving towards enemy at {:?}, distance {}",
                player_index,
                closest_enemy_pos,
                dist
            );
            // Move towards the enemy (but don't attack)
            return execute_move_to(world, player_index, closest_enemy_pos, execution_state);
        } else {
            tracing::debug!("HuntEnemy: Player {} found no enemies", player_index);
        }

        // No enemies visible - move to potential enemy location
        if let Some(closest_potential) = world
            .potential_enemy_locations
            .iter()
            .min_by_key(|pos| player.position.distance(pos))
            .copied()
        {
            tracing::debug!(
                "HuntEnemy: Player {} moving to potential enemy location at {:?}",
                player_index,
                closest_potential
            );
            return execute_move_to(world, player_index, closest_potential, execution_state);
        }

        // No potential locations - pick a random walkable location
        use rand::Rng;
        let mut rng = rand::rng();
        for _ in 0..100 {
            let random_x = rng.random_range(0..world.map.width);
            let random_y = rng.random_range(0..world.map.height);
            let random_pos = crate::infra::Position::new(random_x, random_y);

            if world.is_walkable(&random_pos, random_pos) {
                tracing::debug!(
                    "HuntEnemy: Player {} moving to random location at {:?}",
                    player_index,
                    random_pos
                );
                return execute_move_to(world, player_index, random_pos, execution_state);
            }
        }

        // No walkable random location found - action complete
        tracing::debug!("HuntEnemy: Player {} has no valid targets, completing", player_index);
        (DirectedAction::None, ExecutionStatus::Complete)
    }

    fn cost(&self, state: &GameState, player_index: usize) -> f32 {
        let world = &state.world;
        let player = &world.players[player_index];

        // Calculate cost based on what we would target
        let (distance, is_potential) =
            if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
                let dist = world
                    .find_path_for_player(player_index, player.position, closest_enemy)
                    .map(|p| (p.len() as u32).saturating_sub(1))
                    .unwrap_or(100);
                (dist, false)
            } else if let Some(closest_potential) = world
                .potential_enemy_locations
                .iter()
                .min_by_key(|pos| player.position.distance(pos))
                .copied()
            {
                let dist = world
                    .find_path_for_player(player_index, player.position, closest_potential)
                    .map(|p| (p.len() as u32).saturating_sub(1))
                    .unwrap_or(100);
                (dist, true)
            } else {
                (100, true)
            };

        let base_cost = if is_potential { 25.0 } else { 20.0 };
        base_cost + distance as f32 * 0.1
    }

    fn duration(&self, state: &GameState, player_index: usize) -> u32 {
        let world = &state.world;
        let player = &world.players[player_index];

        // Calculate duration based on what we would target
        if let Some(closest_enemy) = world.enemies.closest_to(player.position) {
            world
                .find_path_for_player(player_index, player.position, closest_enemy)
                .map(|p| p.len() as u32)
                .unwrap_or(100)
        } else if let Some(closest_potential) = world
            .potential_enemy_locations
            .iter()
            .min_by_key(|pos| player.position.distance(pos))
            .copied()
        {
            world
                .find_path_for_player(player_index, player.position, closest_potential)
                .map(|p| p.len() as u32)
                .unwrap_or(100)
        } else {
            100
        }
    }

    fn name(&self) -> String {
        "HuntEnemy".to_string()
    }

    fn reward(&self, state: &GameState, player_index: usize) -> f32 {
        let player = &state.world.players[player_index];
        let world = &state.world;

        // Reward for clearing enemies, higher when maze is fully explored
        let base_reward = if player.unexplored_frontier.is_empty() {
            // Fully explored - prioritize hunting
            20.0
        } else {
            // Still exploring - lower priority
            5.0
        };

        // Penalize if targeting potential enemies (less certain)
        let is_potential = world.enemies.is_empty();
        if is_potential {
            base_reward * 0.5
        } else {
            base_reward
        }
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn is_combat_action(&self) -> bool {
        true
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
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
        if action.precondition(state, player_index) {
            actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
        }

        actions
    }
}
