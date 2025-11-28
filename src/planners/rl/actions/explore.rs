//! Explore action - move toward unexplored frontier

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, ObjectCounts, RLActionTrait};

#[derive(Debug, Clone)]
pub struct ExploreAction {
    pub cached_distance: u32,
}

impl ExploreAction {
    fn find_nearest_frontier(player: &crate::state::PlayerState) -> Option<Position> {
        player
            .unexplored_frontier
            .iter()
            .min_by_key(|p| player.position.distance(p))
            .copied()
    }

    fn count_objects(world: &WorldState) -> ObjectCounts {
        ObjectCounts {
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

impl RLActionTrait for ExploreAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        !world.players[player_index].unexplored_frontier.is_empty()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let current_dest = player.current_destination;
        let frontier = player.unexplored_frontier.clone();

        // Check if we have a current destination
        if let Some(current_dest) = current_dest {
            if player_pos != current_dest {
                if let Some(tile) = world.map.get(&current_dest) {
                    if matches!(
                        tile,
                        crate::swoq_interface::Tile::Unknown | crate::swoq_interface::Tile::Empty
                    ) {
                        if world.find_path(player_pos, current_dest).is_some() {
                            return Some(current_dest);
                        }
                    }
                } else {
                    if world.find_path(player_pos, current_dest).is_some() {
                        return Some(current_dest);
                    }
                }
            }
        }

        // Find nearest reachable frontier as new target
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

        if player.unexplored_frontier.is_empty() {
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        let Some(target) = player.current_destination else {
            return (DirectedAction::None, ExecutionStatus::Complete);
        };

        // Initialize on first execution
        if execution_state.initial_object_counts.is_none() {
            execution_state.initial_object_counts = Some(Self::count_objects(world));
        }

        // Check if new objects have been discovered
        if let Some(ref initial_counts) = execution_state.initial_object_counts {
            let current_counts = Self::count_objects(world);

            let new_objects_discovered = current_counts.num_keys > initial_counts.num_keys
                || current_counts.num_swords > initial_counts.num_swords
                || current_counts.num_health > initial_counts.num_health
                || current_counts.num_pressure_plates > initial_counts.num_pressure_plates
                || current_counts.num_boulders > initial_counts.num_boulders
                || (current_counts.exit_visible && !initial_counts.exit_visible);

            if new_objects_discovered {
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

    fn name(&self) -> String {
        "Explore".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::Explore as usize
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let player = &world.players[player_index];

        if let Some(nearest) = Self::find_nearest_frontier(player) {
            if let Some(path) = world.find_path(player.position, nearest) {
                let action = ExploreAction {
                    cached_distance: path.len() as u32,
                };
                if action.precondition(world, player_index) {
                    return vec![Box::new(action)];
                }
            }
        }
        vec![]
    }
}
