use crate::infra::{Position, path_to_action, use_direction};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::{ActionExecutionState, ExecutionStatus};

pub(super) fn execute_move_to(
    world: &WorldState,
    player_index: usize,
    target: Position,
    execution_state: &mut ActionExecutionState,
) -> (DirectedAction, ExecutionStatus) {
    let player = &world.players[player_index];
    if player.position == target {
        execution_state.cached_path = None;
        execution_state.path_target = None;
        return (DirectedAction::None, ExecutionStatus::Complete);
    }

    // Need to recompute path
    if let Some(path) = world.find_path_for_player(player_index, player.position, target) {
        execution_state.cached_path = Some(path.clone());
        execution_state.path_target = Some(target);

        if let Some(action) = path_to_action(player.position, &path) {
            // Advance the path by removing the current position
            if let Some(cached) = &mut execution_state.cached_path
                && !cached.is_empty()
                && cached[0] == player.position
            {
                cached.remove(0);
            }
            (action, ExecutionStatus::InProgress)
        } else {
            execution_state.cached_path = None;
            execution_state.path_target = None;
            (DirectedAction::None, ExecutionStatus::Failed)
        }
    } else {
        execution_state.cached_path = None;
        execution_state.path_target = None;
        (DirectedAction::None, ExecutionStatus::Failed)
    }
}

pub(super) fn execute_use_adjacent(
    world: &WorldState,
    player_index: usize,
    target: Position,
    execution_state: &mut ActionExecutionState,
) -> (DirectedAction, ExecutionStatus) {
    let player = &world.players[player_index];
    if player.position.is_adjacent(&target) {
        execution_state.cached_path = None;
        execution_state.path_target = None;
        let use_action = use_direction(player.position, target);
        return (use_action, ExecutionStatus::Complete);
    }

    // Use the common execute_move_to which handles path caching
    execute_move_to(world, player_index, target, execution_state)
}
