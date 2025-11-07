use tracing::debug;

use crate::infra::Position;
use crate::planners::heuristic::planner_state::PlannerState;

// Goal modules
pub mod goal;

pub mod avoid_enemy;
pub mod drop_boulder;
pub mod drop_boulder_on_plate;
pub mod explore;
pub mod fetch_boulder;
pub mod get_key;
pub mod kill_enemy;
pub mod open_door;
pub mod pass_through_door;
pub mod pickup_health;
pub mod pickup_sword;
pub mod random_explore;
pub mod reach_exit;
pub mod wait_on_tile;

// Re-export commonly used types
pub use goal::Goal;

// ============================================================================
// Validation functions
// ============================================================================

/// Helper function to determine if a new path should replace the current path
/// Returns true if the new path is shorter than the old path (which is already trimmed)
#[allow(dead_code)]
pub fn should_update_path(new_path: &[Position], old_path: Option<&Vec<Position>>) -> bool {
    if let Some(old_path) = old_path {
        let is_shorter = new_path.len() < old_path.len();

        if is_shorter {
            debug!(
                "New path ({} steps) is shorter than current path ({} steps)",
                new_path.len(),
                old_path.len()
            );
        }
        is_shorter
    } else {
        true
    }
}

/// Step 1: Clear destination and path if goal has changed (for Explore goal)
pub fn clear_path_on_goal_change(
    state: &mut PlannerState,
    player_index: usize,
    current_goal: &Goal,
) {
    let goal_changed =
        state.player_states[player_index].previous_goal.as_ref() != Some(current_goal);
    if goal_changed {
        debug!("Goal changed, clearing destination and path");
        state.world.players[player_index].current_destination = None;
        state.world.players[player_index].current_path = None;
    }
}

/// Step 2: Validate destination - clear if it's no longer empty/unknown
pub fn validate_destination(state: &mut PlannerState, player_index: usize) {
    if let Some(dest) = state.world.players[player_index].current_destination
        && let Some(tile) = state.world.map.get(&dest)
    {
        let is_not_empty = !matches!(
            tile,
            crate::swoq_interface::Tile::Empty | crate::swoq_interface::Tile::Unknown
        );
        if is_not_empty {
            debug!("Destination {:?} is now {:?}, clearing destination and path", dest, tile);
            state.world.players[player_index].current_destination = None;
            state.world.players[player_index].current_path = None;
        }
    }
}

pub fn try_keep_destination(state: &mut PlannerState, player_index: usize) -> bool {
    let player_pos = state.world.players[player_index].position;
    if let Some(dest) = state.world.players[player_index].current_destination {
        if let Some(new_path) = state
            .world
            .find_path_for_player(player_index, player_pos, dest)
        {
            debug!("Continuing to existing destination {:?}, path length={}", dest, new_path.len());
            state.world.players[player_index].current_path = Some(new_path);
            return true;
        }
        debug!("Destination {:?} is now unreachable, finding new one", dest);
        state.world.players[player_index].current_destination = None;
    }
    false
}
