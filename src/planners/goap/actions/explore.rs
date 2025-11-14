use crate::planners::goap::actions::helpers::execute_move_to;
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct ExploreAction {
    pub cached_distance: u32, // Distance to nearest frontier
}

impl ExploreAction {
    fn find_nearest_frontier(player: &crate::state::PlayerState) -> Option<crate::infra::Position> {
        player
            .unexplored_frontier
            .iter()
            .min_by_key(|p| player.position.distance(p))
            .copied()
    }

    fn count_objects(world: &WorldState) -> super::ObjectCounts {
        super::ObjectCounts {
            num_keys: [
                crate::infra::Color::Red,
                crate::infra::Color::Green,
                crate::infra::Color::Blue,
            ]
            .iter()
            .filter_map(|color| world.keys.get_positions(*color))
            .map(|positions| positions.len())
            .sum(),
            num_swords: world.swords.get_positions().len(),
            num_health: world.health.get_positions().len(),
            num_pressure_plates: [
                crate::infra::Color::Red,
                crate::infra::Color::Green,
                crate::infra::Color::Blue,
            ]
            .iter()
            .filter_map(|color| world.pressure_plates.get_positions(*color))
            .map(|positions| positions.len())
            .sum(),
            num_boulders: world.boulders.len(),
            exit_visible: world.exit_position.is_some(),
        }
    }
}

impl GOAPActionTrait for ExploreAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        !state.world.players[player_index]
            .unexplored_frontier
            .is_empty()
    }

    fn effect(&self, _state: &mut GameState, _player_index: usize) {
        // No effect during planning simulation
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];

        tracing::trace!(
            player_index = player_index,
            tick = world.tick,
            frontier_size = player.unexplored_frontier.len(),
            "ExploreAction::execute"
        );

        // Check if frontier is empty
        if player.unexplored_frontier.is_empty() {
            tracing::debug!(player_index = player_index, "Frontier empty, exploration complete");
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        // Initialize object counts on first execution
        if execution_state.initial_object_counts.is_none() {
            let initial = Self::count_objects(world);
            tracing::debug!(
                player_index = player_index,
                initial_keys = initial.num_keys,
                initial_swords = initial.num_swords,
                initial_health = initial.num_health,
                initial_plates = initial.num_pressure_plates,
                initial_boulders = initial.num_boulders,
                initial_exit = initial.exit_visible,
                "Initialized object counts for exploration"
            );
            execution_state.initial_object_counts = Some(initial);
        }

        // Check if new objects have been discovered
        if let Some(ref initial_counts) = execution_state.initial_object_counts {
            let current_counts = Self::count_objects(world);

            tracing::trace!(
                player_index = player_index,
                initial_keys = initial_counts.num_keys,
                current_keys = current_counts.num_keys,
                initial_swords = initial_counts.num_swords,
                current_swords = current_counts.num_swords,
                initial_health = initial_counts.num_health,
                current_health = current_counts.num_health,
                initial_plates = initial_counts.num_pressure_plates,
                current_plates = current_counts.num_pressure_plates,
                initial_boulders = initial_counts.num_boulders,
                current_boulders = current_counts.num_boulders,
                initial_exit = initial_counts.exit_visible,
                current_exit = current_counts.exit_visible,
                "Checking for new objects"
            );

            let new_objects_discovered = current_counts.num_keys > initial_counts.num_keys
                || current_counts.num_swords > initial_counts.num_swords
                || current_counts.num_health > initial_counts.num_health
                || current_counts.num_pressure_plates > initial_counts.num_pressure_plates
                || current_counts.num_boulders > initial_counts.num_boulders
                || (current_counts.exit_visible && !initial_counts.exit_visible);

            if new_objects_discovered {
                tracing::info!(
                    player_index = player_index,
                    keys_change = current_counts.num_keys as i32 - initial_counts.num_keys as i32,
                    swords_change =
                        current_counts.num_swords as i32 - initial_counts.num_swords as i32,
                    health_change =
                        current_counts.num_health as i32 - initial_counts.num_health as i32,
                    plates_change = current_counts.num_pressure_plates as i32
                        - initial_counts.num_pressure_plates as i32,
                    boulders_change =
                        current_counts.num_boulders as i32 - initial_counts.num_boulders as i32,
                    exit_discovered = current_counts.exit_visible && !initial_counts.exit_visible,
                    "NEW OBJECTS DISCOVERED! Completing exploration action"
                );
                execution_state.exploration_target = None;
                execution_state.initial_object_counts = None;
                return (DirectedAction::None, ExecutionStatus::Complete);
            }
        }

        // Determine target: use cached target if still valid, otherwise find nearest
        let needs_new_target = execution_state
            .exploration_target
            .is_none_or(|pos| !player.unexplored_frontier.contains(&pos));

        if needs_new_target {
            let nearest = Self::find_nearest_frontier(player).unwrap();
            execution_state.exploration_target = Some(nearest);
        }

        let target = execution_state.exploration_target.unwrap();

        let result = execute_move_to(world, player_index, target, execution_state);
        if result.1 == ExecutionStatus::Complete || result.1 == ExecutionStatus::Failed {
            execution_state.exploration_target = None;
            execution_state.initial_object_counts = None;
        }
        result
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        1.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance
    }

    fn name(&self) -> &'static str {
        "Explore"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let world = &state.world;
        let player = &world.players[player_index];

        // Find nearest frontier and cache the distance
        if let Some(nearest) = Self::find_nearest_frontier(player)
            && let Some(path) = world.find_path_for_player(player_index, player.position, nearest)
        {
            let action = ExploreAction {
                cached_distance: path.len() as u32,
            };
            if action.precondition(state, player_index) {
                return vec![Box::new(action)];
            }
        }
        vec![]
    }
}
