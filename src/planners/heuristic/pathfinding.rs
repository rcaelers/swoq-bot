//! Player-aware pathfinding for the heuristic planner.
//!
//! These functions handle multi-player collision avoidance when finding paths.

use tracing::debug;

use crate::infra::{AStar, Color, Position};
use crate::state::{Map, WorldState};
use crate::swoq_interface::Tile;

/// Check if a position is walkable for a specific player
/// If planning_player_pos is provided, doors won't be considered open if that player
/// is the only one on the pressure plate (since they'll leave it to move)
/// If goal is None, items/doors/unknown tiles are treated as non-walkable
fn is_walkable_for_player(
    world: &WorldState,
    pos: &Position,
    goal: Option<Position>,
    planning_player_pos: Option<Position>,
) -> bool {
    match world.map.get(pos) {
        Some(
            Tile::Empty
            | Tile::Player
            | Tile::PressurePlateRed
            | Tile::PressurePlateGreen
            | Tile::PressurePlateBlue
            | Tile::Treasure,
        ) => true,
        // Doors are walkable if their corresponding pressure plate is pressed
        // (but not if the planning player is the only one on the plate)
        // Also allow doors if they are the goal destination (for OpenDoor action)
        Some(Tile::DoorRed) => {
            goal.is_some_and(|g| *pos == g)
                || world.is_door_open_for_player(Color::Red, planning_player_pos)
        }
        Some(Tile::DoorGreen) => {
            goal.is_some_and(|g| *pos == g)
                || world.is_door_open_for_player(Color::Green, planning_player_pos)
        }
        Some(Tile::DoorBlue) => {
            goal.is_some_and(|g| *pos == g)
                || world.is_door_open_for_player(Color::Blue, planning_player_pos)
        }
        // Keys: always avoid unless it's the destination
        Some(
            Tile::KeyRed
            | Tile::KeyGreen
            | Tile::KeyBlue
            | Tile::Sword
            | Tile::Health
            | Tile::Exit
            | Tile::Enemy
            | Tile::Unknown,
        ) => {
            // Allow walking on the destination key/item/enemy, avoid all others
            goal.is_some_and(|g| *pos == g)
        }
        None => goal.is_some_and(|g| *pos == g),
        _ => false,
    }
}

/// Find a path that avoids colliding with another player's planned path
fn find_path_avoiding_player(
    world: &WorldState,
    map: &Map,
    start: Position,
    goal: Position,
    other_player_path: &[Position],
    planning_player_pos: Position,
) -> Option<Vec<Position>> {
    // Check if the start position itself conflicts with the other player's path
    // At tick 0, we can't start where the other player is
    if !other_player_path.is_empty() && start == other_player_path[0] {
        debug!("  ✗ Start position {:?} conflicts with other player at tick 0", start);
        return None;
    }

    // Check if start position is where the other player will be at tick 1 (swap collision)
    if other_player_path.len() > 1 && start == other_player_path[1] {
        debug!(
            "  ✗ Start position {:?} would cause swap collision with other player at tick 1",
            start
        );
        return None;
    }

    AStar::find_path_with_cost(
        map,
        start,
        goal,
        |pos, goal_pos, tick| {
            // First check basic walkability (including door states)
            // Pass the planning player's position so doors they're holding open aren't considered walkable
            if !is_walkable_for_player(world, pos, Some(goal_pos), Some(planning_player_pos)) {
                return false;
            }

            let tick_index = tick as usize;

            // Check if the other player is at this position at this tick
            if tick_index < other_player_path.len() {
                if *pos == other_player_path[tick_index] {
                    return false;
                }
            } else if let Some(last_pos) = other_player_path.last()
                && *pos == *last_pos
            {
                return false;
            }

            // Check swap collisions
            if tick_index > 0
                && tick_index - 1 < other_player_path.len()
                && *pos == other_player_path[tick_index - 1]
            {
                return false;
            }

            if tick_index + 1 < other_player_path.len() && *pos == other_player_path[tick_index + 1]
            {
                return false;
            }

            true
        },
        |pos, _goal_pos, _tick| {
            // Use enemy-aware movement cost
            world.movement_cost(pos)
        },
    )
}

/// Find a path for a player, avoiding collision with other player's path
pub fn find_path_for_player(
    world: &WorldState,
    player_index: usize,
    start: Position,
    goal: Position,
) -> Option<Vec<Position>> {
    let planning_player_pos = world.players[player_index].position;

    // For player 2, avoid player 1's path or position
    if player_index == 1 && world.players.len() > 1 {
        debug!(
            "Pathfinding context: Player 0 at {:?}, Player 1 at {:?}",
            world.players[0].position, world.players[1].position
        );

        // Determine what to avoid: either player 1's path, or their static position
        let (p1_path_to_avoid, is_static) = if let Some(ref p1_path) = world.players[0].current_path
        {
            (p1_path.clone(), false)
        } else {
            // Player 1 has no path, treat their current position as permanently blocked
            // Single-element path means the position is blocked at all future ticks
            // (find_path_avoiding_player checks last_pos for ticks beyond path length)
            (vec![world.players[0].position], true)
        };

        if is_static {
            debug!(
                "Player {} (index {}) finding path from {:?} to {:?}, avoiding Player {} (index {}) static position {:?}",
                player_index + 1,
                player_index,
                start,
                goal,
                1,
                0,
                p1_path_to_avoid[0]
            );
        } else {
            debug!(
                "Player {} (index {}) finding path from {:?} to {:?}, avoiding Player {} (index {}) path (length: {})",
                player_index + 1,
                player_index,
                start,
                goal,
                1,
                0,
                p1_path_to_avoid.len()
            );
        }

        let result = find_path_avoiding_player(
            world,
            &world.map,
            start,
            goal,
            &p1_path_to_avoid,
            planning_player_pos,
        );
        if result.is_some() {
            if is_static {
                debug!("  ✓ Found path avoiding Player 1's position");
            } else {
                debug!("  ✓ Found path avoiding Player 1");
                debug!("    Player 1 path: {:?}", p1_path_to_avoid);
                debug!("    Player 2 path: {:?}", result);
            }
        } else {
            debug!("  ✗ No direct path found avoiding Player 1, selecting random destination");

            // Try to find a random reachable position within Manhattan distance 20
            const MAX_DISTANCE: i32 = 20;
            const MAX_ATTEMPTS: usize = 50;

            for attempt in 0..MAX_ATTEMPTS {
                // Generate random offset within Manhattan distance
                let seed = (world.tick as usize)
                    .wrapping_mul(1103515245)
                    .wrapping_add(12345)
                    .wrapping_add(attempt);
                let dx = ((seed % (MAX_DISTANCE * 2 + 1) as usize) as i32) - MAX_DISTANCE;
                let dy_range = MAX_DISTANCE - dx.abs();
                let dy = ((seed.wrapping_mul(31) % (dy_range * 2 + 1) as usize) as i32) - dy_range;

                let random_pos = Position::new(start.x + dx, start.y + dy);

                // Check if position is in bounds and walkable
                if random_pos.x >= 0
                    && random_pos.x < world.map.width
                    && random_pos.y >= 0
                    && random_pos.y < world.map.height
                    && is_walkable_for_player(
                        world,
                        &random_pos,
                        Some(random_pos),
                        Some(planning_player_pos),
                    )
                {
                    // Try to find path to this random position, still avoiding player 1
                    if let Some(path) = find_path_avoiding_player(
                        world,
                        &world.map,
                        start,
                        random_pos,
                        &p1_path_to_avoid,
                        planning_player_pos,
                    ) {
                        debug!(
                            "  ✓ Found path to random destination {:?} (attempt {})",
                            random_pos,
                            attempt + 1
                        );
                        return Some(path);
                    }
                }
            }

            debug!(
                "  ✗ Could not find path to any random destination after {} attempts",
                MAX_ATTEMPTS
            );
        }
        return result;
    }

    // For player 1 or when player 1 has no path, use regular pathfinding
    world.find_path(start, goal)
}

/// Find a path with custom walkability checking logic.
/// The closure receives (position, goal, tick) and should return true if the position is walkable.
/// Also avoids tiles adjacent to enemies.
pub fn find_path_with_custom_walkability<F>(
    world: &WorldState,
    start: Position,
    goal: Position,
    is_walkable: F,
) -> Option<Vec<Position>>
where
    F: Fn(&Position, Position, i32) -> bool,
{
    AStar::find_path_with_cost(&world.map, start, goal, is_walkable, |pos, _goal_pos, _tick| {
        world.movement_cost(pos)
    })
}
