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
            tracing::debug!(
                "execute_move_to: Player {} could not convert path to action from {:?} to {:?}",
                player_index,
                player.position,
                target
            );
            execution_state.cached_path = None;
            execution_state.path_target = None;
            (DirectedAction::None, ExecutionStatus::Failed)
        }
    } else {
        tracing::debug!(
            "execute_move_to: Player {} could not find path from {:?} to {:?}",
            player_index,
            player.position,
            target
        );
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

pub(super) fn execute_avoid(
    world: &WorldState,
    player_index: usize,
    danger_pos: Position,
) -> (DirectedAction, ExecutionStatus) {
    let player = &world.players[player_index];
    let player_pos = player.position;
    let current_distance = player_pos.distance(&danger_pos);

    // Try all four directions and pick the one that maximizes distance from danger
    let mut best_action = None;
    let mut best_distance = current_distance;

    let actions = [
        (DirectedAction::MoveNorth, Position::new(player_pos.x, player_pos.y - 1)),
        (DirectedAction::MoveEast, Position::new(player_pos.x + 1, player_pos.y)),
        (DirectedAction::MoveSouth, Position::new(player_pos.x, player_pos.y + 1)),
        (DirectedAction::MoveWest, Position::new(player_pos.x - 1, player_pos.y)),
    ];

    for (action, new_pos) in actions {
        // Only consider walkable positions
        if !world.is_walkable(&new_pos, danger_pos) {
            continue;
        }

        let dist = new_pos.distance(&danger_pos);
        if dist > best_distance {
            best_distance = dist;
            best_action = Some(action);
        }
    }

    if let Some(action) = best_action {
        (action, ExecutionStatus::InProgress)
    } else {
        // No walkable move found that increases distance - stay in place
        (DirectedAction::None, ExecutionStatus::Complete)
    }
}
