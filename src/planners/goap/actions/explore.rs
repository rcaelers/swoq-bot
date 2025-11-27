use crate::planners::goap::actions::helpers::execute_move_to;
use crate::planners::goap::game_state::PlanningState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct ExploreAction {
    pub cached_distance: u32, // Distance to nearest frontier (for cost/duration)
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
    fn precondition(
        &self,
        world: &WorldState,
        _state: &PlanningState,
        player_index: usize,
    ) -> bool {
        !world.players[player_index].unexplored_frontier.is_empty()
    }

    fn effect_end(
        &self,
        _world: &mut WorldState,
        _state: &mut PlanningState,
        _player_index: usize,
    ) {
        // No effect during planning simulation
    }

    fn prepare(
        &mut self,
        world: &mut WorldState,
        player_index: usize,
    ) -> Option<crate::infra::Position> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let current_dest = player.current_destination;
        let frontier = player.unexplored_frontier.clone();

        // Check if we have a current destination
        if let Some(current_dest) = current_dest {
            // Check if destination is still valid (unknown, Unknown, or empty, and not reached yet)
            if player_pos != current_dest {
                if let Some(tile) = world.map.get(&current_dest) {
                    // Tile is known - check if it's Unknown or empty
                    if matches!(
                        tile,
                        crate::swoq_interface::Tile::Unknown | crate::swoq_interface::Tile::Empty
                    ) {
                        // Check if still reachable
                        if world.find_path(player_pos, current_dest).is_some() {
                            return Some(current_dest);
                        }
                        // No longer reachable, find new target
                    }
                    // Tile became non-empty and not Unknown, find new target
                } else {
                    // Tile is not in map (unknown), check if reachable
                    if world.find_path(player_pos, current_dest).is_some() {
                        return Some(current_dest);
                    }
                    // No longer reachable, find new target
                }
            }
            // Destination reached or became non-empty/non-Unknown, find new one
        }

        // Find nearest reachable frontier as new target
        // Iterate through frontier sorted by distance and return first reachable one
        let mut frontier_by_distance: Vec<_> = frontier
            .iter()
            .map(|&pos| (player_pos.distance(&pos), pos))
            .collect();
        frontier_by_distance.sort_by_key(|(dist, _)| *dist);

        frontier_by_distance
            .into_iter()
            .map(|(_, frontier_pos)| frontier_pos)
            .find(|&frontier_pos| world.find_path(player_pos, frontier_pos).is_some())
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

        // Get target from current_destination (set by prepare)
        let Some(target) = player.current_destination else {
            tracing::debug!(
                "Explore: Player {} has no current destination, completing",
                player_index
            );
            return (DirectedAction::None, ExecutionStatus::Complete);
        };

        // Initialize on first execution
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
                execution_state.initial_object_counts = None;
                return (DirectedAction::None, ExecutionStatus::Complete);
            }
        }

        let result = execute_move_to(world, player_index, target, execution_state);
        if result.1 == ExecutionStatus::Complete || result.1 == ExecutionStatus::Failed {
            execution_state.initial_object_counts = None;
        }
        result
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        1.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.cached_distance
    }

    fn name(&self) -> String {
        "Explore".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn reward(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        // Small reward for exploration action to encourage discovering new areas
        1.0
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let world = &world;
        let player = &world.players[player_index];

        tracing::trace!(
            player_index = player_index,
            unexplored_frontier_size = player.unexplored_frontier.len(),
            "Generating ExploreAction"
        );
        // Find nearest frontier and cache the distance for cost/duration
        if let Some(nearest) = Self::find_nearest_frontier(player) {
            tracing::trace!(
                player_index = player_index,
                nearest_frontier = ?nearest,
                "Generating ExploreAction"
            );
            if let Some(path) = world.find_path(player.position, nearest) {
                tracing::trace!(
                    player_index = player_index,
                    path_length = path.len(),
                    "Found path to nearest frontier"
                );
                let action = ExploreAction {
                    cached_distance: path.len() as u32,
                };
                if action.precondition(world, state, player_index) {
                    return vec![Box::new(action)];
                }
            }
        }
        vec![]
    }
}
