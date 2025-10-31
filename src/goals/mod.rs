use tracing::debug;

use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

// Goal modules
pub mod goal;

pub mod explore;
pub mod get_key;
pub mod open_door;
pub mod wait_on_tile;
pub mod pass_through_door;
pub mod pickup_sword;
pub mod pickup_health;
pub mod avoid_enemy;
pub mod kill_enemy;
pub mod fetch_boulder;
pub mod drop_boulder;
pub mod drop_boulder_on_plate;
pub mod reach_exit;
pub mod random_explore;

// Re-export commonly used types
pub use goal::Goal;

// ============================================================================
// Helper functions 
// ============================================================================

pub fn path_to_action(current: Position, path: &[Position]) -> Option<DirectedAction> {
    if path.len() < 2 {
        return None;
    }
    let next = path[1];

    if next.y < current.y {
        Some(DirectedAction::MoveNorth)
    } else if next.y > current.y {
        Some(DirectedAction::MoveSouth)
    } else if next.x > current.x {
        Some(DirectedAction::MoveEast)
    } else if next.x < current.x {
        Some(DirectedAction::MoveWest)
    } else {
        None
    }
}

pub fn use_direction(from: Position, to: Position) -> DirectedAction {
    if to.y < from.y {
        DirectedAction::UseNorth
    } else if to.y > from.y {
        DirectedAction::UseSouth
    } else if to.x > from.x {
        DirectedAction::UseEast
    } else {
        DirectedAction::UseWest
    }
}

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
pub fn clear_path_on_goal_change(world: &mut WorldState, player_index: usize, current_goal: &Goal) {
    let goal_changed = world.players[player_index].previous_goal.as_ref() != Some(current_goal);
    if goal_changed {
        debug!("Goal changed, clearing destination and path");
        world.players[player_index].current_destination = None;
        world.players[player_index].current_path = None;
    }
}

/// Step 2: Validate destination - clear if it's no longer empty/unknown
pub fn validate_destination(world: &mut WorldState, player_index: usize) {
    if let Some(dest) = world.players[player_index].current_destination
        && let Some(tile) = world.map.get(&dest)
    {
        let is_not_empty = !matches!(
            tile,
            crate::swoq_interface::Tile::Empty | crate::swoq_interface::Tile::Unknown
        );
        if is_not_empty {
            debug!("Destination {:?} is now {:?}, clearing destination and path", dest, tile);
            world.players[player_index].current_destination = None;
            world.players[player_index].current_path = None;
        }
    }
}

/// Step 3: Trim and validate path - check if old path is still walkable and ends at destination
pub fn validate_and_trim_path(world: &mut WorldState, player_index: usize) {
    let player_pos = world.players[player_index].position;

    if let Some(dest) = world.players[player_index].current_destination
        && let Some(ref old_path) = world.players[player_index].current_path
    {
        // Skip positions we've already passed - find our current position in the path
        let remaining_path: Vec<_> = old_path
            .iter()
            .skip_while(|&&pos| pos != player_pos)
            .copied()
            .collect();

        let path_valid = !remaining_path.is_empty()
            && remaining_path.last() == Some(&dest)
            && remaining_path
                .iter()
                .all(|&pos| world.map.is_walkable(&pos, dest));

        if !path_valid {
            debug!("Old path is no longer valid, clearing but keeping destination");
            world.players[player_index].current_path = None;
        } else if remaining_path.len() < old_path.len() {
            // Update path to trimmed version
            world.players[player_index].current_path = Some(remaining_path);
        }
    }
}

/// Helper for ExploreGoal: Try to update path to existing destination
#[allow(dead_code)]
pub fn try_update_path_to_destination(world: &mut WorldState, player_index: usize) -> bool {
    let player_pos = world.players[player_index].position;

    if let Some(dest) = world.players[player_index].current_destination
        && let Some(new_path) = world.find_path_for_player(player_index, player_pos, dest)
    {
        if should_update_path(&new_path, world.players[player_index].current_path.as_ref()) {
            debug!("Updating path to destination {:?}, new path length={}", dest, new_path.len());
            world.players[player_index].current_path = Some(new_path);
        } else {
            debug!("Keeping existing path to destination {:?}", dest);
        }
        return true;
    }
    false
}

pub fn try_keep_destination(world: &mut WorldState, player_index: usize) -> bool {
    let player_pos = world.players[player_index].position;
    if let Some(dest) = world.players[player_index].current_destination {
        if let Some(new_path) = world.find_path_for_player(player_index, player_pos, dest) {
            debug!("Continuing to existing destination {:?}, path length={}", dest, new_path.len());
            world.players[player_index].current_path = Some(new_path);
            return true;
        }
        debug!("Destination {:?} is now unreachable, finding new one", dest);
        world.players[player_index].current_destination = None;
    }
    false
}
