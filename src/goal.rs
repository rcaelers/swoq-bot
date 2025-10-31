use tracing::debug;

use crate::swoq_interface::DirectedAction;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

/// Helper function to determine if a new path should replace the current path
/// Returns true if the new path is shorter than the old path (which is already trimmed)
#[allow(dead_code)]
fn should_update_path(new_path: &[Position], old_path: Option<&Vec<Position>>) -> bool {
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
fn clear_path_on_goal_change(world: &mut WorldState, player_index: usize, current_goal: &Goal) {
    let goal_changed = world.players[player_index].previous_goal.as_ref() != Some(current_goal);
    if goal_changed {
        debug!("Goal changed, clearing destination and path");
        world.players[player_index].current_destination = None;
        world.players[player_index].current_path = None;
    }
}

/// Step 2: Validate destination - clear if it's no longer empty/unknown
fn validate_destination(world: &mut WorldState, player_index: usize) {
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
fn validate_and_trim_path(world: &mut WorldState, player_index: usize) {
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
fn try_update_path_to_destination(world: &mut WorldState, player_index: usize) -> bool {
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

fn try_keep_destination(world: &mut WorldState, player_index: usize) -> bool {
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

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Explore,
    GetKey(Color),
    OpenDoor(Color),
    WaitOnPressurePlate(Color, Position),
    PassThroughDoor(Color, Position, Position), // door_pos, target_pos (beyond door)
    PickupSword,
    PickupHealth(Position),
    AvoidEnemy(Position),
    KillEnemy(Position),
    FetchBoulder(Position),
    DropBoulder,
    DropBoulderOnPlate(Color, Position),
    ReachExit,
    RandomExplore(Position),
}

impl Goal {
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        clear_path_on_goal_change(world, player_index, self);

        // Delegate to specific goal implementation
        match self {
            Goal::Explore => ExploreGoal.execute(world, player_index),
            Goal::GetKey(color) => GetKeyGoal(*color).execute(world, player_index),
            Goal::OpenDoor(color) => OpenDoorGoal(*color).execute(world, player_index),
            Goal::WaitOnPressurePlate(_color, pos) => {
                WaitOnPressurePlateGoal(*pos).execute(world, player_index)
            }
            Goal::PassThroughDoor(_color, door_pos, target_pos) => {
                PassThroughDoorGoal(*door_pos, *target_pos).execute(world, player_index)
            }
            Goal::PickupSword => PickupSwordGoal.execute(world, player_index),
            Goal::PickupHealth(pos) => PickupHealthGoal(*pos).execute(world, player_index),
            Goal::ReachExit => ReachExitGoal.execute(world, player_index),
            Goal::KillEnemy(pos) => KillEnemyGoal(*pos).execute(world, player_index),
            Goal::AvoidEnemy(pos) => AvoidEnemyGoal(*pos).execute(world, player_index),
            Goal::FetchBoulder(pos) => FetchBoulderGoal(*pos).execute(world, player_index),
            Goal::DropBoulderOnPlate(_color, pos) => {
                DropBoulderOnPlateGoal(*pos).execute(world, player_index)
            }
            Goal::DropBoulder => DropBoulderGoal.execute(world, player_index),
            Goal::RandomExplore(pos) => RandomExploreGoal(*pos).execute(world, player_index),
        }
    }

    /// Execute goal for a specific player (convenience wrapper)
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn execute_for_player(
        &self,
        world: &mut WorldState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        self.execute(world, player_index)
    }
}

pub trait ExecuteGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction>;
}

struct ExploreGoal;
struct GetKeyGoal(Color);
struct OpenDoorGoal(Color);
struct WaitOnPressurePlateGoal(Position);
struct PassThroughDoorGoal(Position, Position); // door_pos, target_pos
struct PickupSwordGoal;
struct PickupHealthGoal(Position);
struct AvoidEnemyGoal(Position);
struct KillEnemyGoal(Position);
struct FetchBoulderGoal(Position);
struct DropBoulderGoal;
struct DropBoulderOnPlateGoal(Position);
struct ReachExitGoal;
struct RandomExploreGoal(Position);

impl ExecuteGoal for ExploreGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = world.players[player_index].position;

        // Step 1: Validate destination
        validate_destination(world, player_index);

        // Step 2: Try to reuse existing destination
        if try_keep_destination(world, player_index) {
            return path_to_action(player_pos, world.players[player_index].current_path.as_ref()?);
        }

        // Step 3: Search for new frontier destination
        let sorted_frontier = &world.players[player_index].sorted_unexplored();
        debug!("Searching for new frontier destination from {} tiles", sorted_frontier.len());
        let mut attempts = 0;
        for (i, target) in sorted_frontier.iter().enumerate() {
            if i < 5 {
                debug!(
                    "  Trying frontier #{}: {:?}, distance={}",
                    i,
                    target,
                    player_pos.distance(target)
                );
            }
            attempts += 1;
            if let Some(path) = world.find_path_for_player(player_index, player_pos, *target) {
                debug!(
                    "New frontier destination: {:?}, path length={} (tried {} tiles)",
                    target,
                    path.len(),
                    attempts
                );
                world.players[player_index].current_destination = Some(*target);
                world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!(
            "No reachable frontier tiles found out of {} candidates (tried {} tiles)",
            sorted_frontier.len(),
            attempts
        );
        world.players[player_index].current_destination = None;
        world.players[player_index].current_path = None;

        None
    }
}

impl ExecuteGoal for GetKeyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = world.players[player_index].position;
        let key_pos = world.closest_key(&world.players[player_index], self.0)?;

        // Validate destination and trim path
        validate_destination(world, player_index);
        validate_and_trim_path(world, player_index);

        // Check if we can reuse existing path
        if let Some(dest) = world.players[player_index].current_destination
            && dest == key_pos
            && let Some(ref path) = world.players[player_index].current_path
        {
            return path_to_action(player_pos, path);
        }

        // Compute new path
        world.players[player_index].current_destination = Some(key_pos);
        let path = world.find_path_for_player(player_index, player_pos, key_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
impl ExecuteGoal for OpenDoorGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let door_positions = world.doors.get_positions(self.0)?;

        // OpenDoor is only for keys - if we don't have a key, this shouldn't be selected
        if !world.has_key(&world.players[player_index], self.0) {
            debug!("OpenDoor goal but no {:?} key!", self.0);
            return None;
        }

        // Find the closest reachable door by finding the best empty neighbor
        let mut best_target: Option<(Position, Position, usize)> = None; // (door_pos, neighbor_pos, path_len)

        for &door_pos in door_positions {
            // Check each neighbor of the door
            for neighbor in door_pos.neighbors() {
                // Only consider empty, walkable neighbors (or player position)
                if neighbor != player_pos
                    && !matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                {
                    continue;
                }

                if !world.map.is_walkable(&neighbor, neighbor) {
                    continue;
                }

                // Try to path to this neighbor
                if let Some(path) = world.map.find_path(player_pos, neighbor) {
                    let path_len = path.len();
                    if best_target.is_none() || path_len < best_target.unwrap().2 {
                        best_target = Some((door_pos, neighbor, path_len));
                    }
                }
            }
        }

        let (door_pos, target_pos, _) = best_target?;

        // If the door is adjacent to us, use the key on it
        if player_pos.is_adjacent(&door_pos) {
            debug!("Door is adjacent, using key on door at {:?}", door_pos);
            return Some(use_direction(player_pos, door_pos));
        }

        // Navigate to the empty neighbor of the door
        debug!("Navigating to neighbor {:?} of door at {:?}", target_pos, door_pos);
        world.players[player_index].current_destination = Some(target_pos);
        let path = world.find_path_for_player(player_index, player_pos, target_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for WaitOnPressurePlateGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let plate_pos = self.0;

        if player_pos == plate_pos {
            // Already on the plate - wait (do nothing)
            debug!("Waiting on pressure plate at {:?}", plate_pos);
            Some(DirectedAction::None)
        } else {
            // Navigate to the pressure plate using collision-aware pathfinding
            debug!("Navigating to pressure plate at {:?} to wait", plate_pos);
            world.players[player_index].current_destination = Some(plate_pos);
            let path = world.find_path_for_player(player_index, player_pos, plate_pos)?;
            world.players[player_index].current_path = Some(path.clone());
            path_to_action(player_pos, &path)
        }
    }
}

impl ExecuteGoal for PassThroughDoorGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let neighbor_pos = self.0; // Position adjacent to door
        let target_pos = self.1; // Position beyond door

        // Calculate door position from neighbor and target
        // Door is between neighbor and target
        let dx = target_pos.x - neighbor_pos.x;
        let dy = target_pos.y - neighbor_pos.y;
        let door_pos = Position {
            x: neighbor_pos.x + dx / 2,
            y: neighbor_pos.y + dy / 2,
        };

        // Check if the door is currently closed (not walkable)
        let door_tile = world.map.get(&door_pos);
        let door_closed = !matches!(door_tile, Some(crate::swoq_interface::Tile::Empty));

        // If we've reached the target position, we're done
        if player_pos == target_pos {
            debug!("Reached target position {:?} beyond door at {:?}", target_pos, door_pos);
            return Some(DirectedAction::None);
        }

        // If we're on the door position, take one more step toward target
        if player_pos == door_pos {
            debug!("On door at {:?}, moving to target {:?}", door_pos, target_pos);
            let dx = target_pos.x - player_pos.x;
            let dy = target_pos.y - player_pos.y;
            return Some(if dy < 0 {
                DirectedAction::MoveNorth
            } else if dy > 0 {
                DirectedAction::MoveSouth
            } else if dx > 0 {
                DirectedAction::MoveEast
            } else {
                DirectedAction::MoveWest
            });
        }

        // If we're at the neighbor position (adjacent to door)
        if player_pos == neighbor_pos {
            if door_closed {
                debug!("At neighbor {:?}, door at {:?} is closed, waiting", neighbor_pos, door_pos);
                return Some(DirectedAction::None);
            } else {
                debug!(
                    "At neighbor {:?}, door at {:?} is open, stepping onto it",
                    neighbor_pos, door_pos
                );
                let dx = door_pos.x - player_pos.x;
                let dy = door_pos.y - player_pos.y;
                return Some(if dy < 0 {
                    DirectedAction::MoveNorth
                } else if dy > 0 {
                    DirectedAction::MoveSouth
                } else if dx > 0 {
                    DirectedAction::MoveEast
                } else {
                    DirectedAction::MoveWest
                });
            }
        }

        // Otherwise, navigate to the neighbor position first
        if let Some(path) = world.find_path_for_player(player_index, player_pos, neighbor_pos) {
            debug!("Navigating to neighbor {:?} before door at {:?}", neighbor_pos, door_pos);
            world.players[player_index].current_destination = Some(neighbor_pos);
            world.players[player_index].current_path = Some(path.clone());
            return path_to_action(player_pos, &path);
        }

        debug!("Cannot find path to neighbor {:?} for door at {:?}", neighbor_pos, door_pos);
        None
    }
}

impl ExecuteGoal for PickupSwordGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = world.players[player_index].position;
        let sword_pos = world.closest_sword(&world.players[player_index])?;

        // Validate destination and trim path
        validate_destination(world, player_index);
        validate_and_trim_path(world, player_index);

        // Check if we can reuse existing path
        if let Some(dest) = world.players[player_index].current_destination
            && dest == sword_pos
            && let Some(ref path) = world.players[player_index].current_path
        {
            return path_to_action(player_pos, path);
        }

        // Compute new path
        world.players[player_index].current_destination = Some(sword_pos);
        let path = world.find_path_for_player(player_index, player_pos, sword_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let health_pos = self.0;
        debug!("PickupHealth: going to destination {:?}", health_pos);
        world.players[player_index].current_destination = Some(health_pos);
        let path = world.find_path_for_player(player_index, player_pos, health_pos)?;
        debug!("PickupHealth: path length={}", path.len());
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for ReachExitGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let exit_pos = world.exit_position?;
        world.players[player_index].current_destination = Some(exit_pos);
        let path = world.find_path_for_player(player_index, player_pos, exit_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for KillEnemyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let enemy_pos = self.0;

        // If adjacent, attack
        if player_pos.is_adjacent(&enemy_pos) {
            return Some(use_direction(player_pos, enemy_pos));
        }

        // Move adjacent to enemy
        for adjacent in enemy_pos.neighbors() {
            if world.map.is_walkable(&adjacent, adjacent)
                && let Some(path) = world.find_path_for_player(player_index, player_pos, adjacent)
            {
                world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        None
    }
}

impl ExecuteGoal for AvoidEnemyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        flee_direction(world, self.0, player_index)
    }
}

impl ExecuteGoal for FetchBoulderGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let boulder_pos = self.0;

        // If we're already adjacent, pick up the boulder
        if player_pos.is_adjacent(&boulder_pos) {
            debug!("Picking up boulder at {:?}", boulder_pos);
            return Some(use_direction(player_pos, boulder_pos));
        }

        // Navigate to an adjacent walkable position next to the boulder
        for adjacent in boulder_pos.neighbors() {
            if world.map.is_walkable(&adjacent, adjacent)
                && let Some(path) = world.find_path_for_player(player_index, player_pos, adjacent)
            {
                debug!("Moving to adjacent position {:?} to reach boulder", adjacent);
                world.players[player_index].current_destination = Some(adjacent);
                world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!("No walkable position adjacent to boulder at {:?}", boulder_pos);
        None
    }
}

impl ExecuteGoal for DropBoulderOnPlateGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let plate_pos = self.0;

        // If we're adjacent to the pressure plate, drop the boulder on it
        if player_pos.is_adjacent(&plate_pos) {
            debug!("Dropping boulder on pressure plate at {:?}", plate_pos);
            return Some(use_direction(player_pos, plate_pos));
        }

        // Navigate to the pressure plate
        world.players[player_index].current_destination = Some(plate_pos);
        let path = world.find_path_for_player(player_index, player_pos, plate_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for DropBoulderGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        // Find a safe place to drop the boulder (empty adjacent tile)
        for neighbor in player_pos.neighbors() {
            if matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                && neighbor.x >= 0
                && neighbor.x < world.map.width
                && neighbor.y >= 0
                && neighbor.y < world.map.height
            {
                debug!("Dropping boulder at {:?}", neighbor);
                return Some(use_direction(player_pos, neighbor));
            }
        }
        // Can't drop anywhere, try to move to find a drop location
        debug!("No adjacent empty tile to drop boulder, trying to move");
        // Try to move in any direction
        for direction in [
            DirectedAction::MoveNorth,
            DirectedAction::MoveEast,
            DirectedAction::MoveSouth,
            DirectedAction::MoveWest,
        ] {
            // Check if the direction is walkable
            let next_pos = match direction {
                DirectedAction::MoveNorth => Position::new(
                    world.players[player_index].position.x,
                    world.players[player_index].position.y - 1,
                ),
                DirectedAction::MoveEast => Position::new(
                    world.players[player_index].position.x + 1,
                    world.players[player_index].position.y,
                ),
                DirectedAction::MoveSouth => Position::new(
                    world.players[player_index].position.x,
                    world.players[player_index].position.y + 1,
                ),
                DirectedAction::MoveWest => Position::new(
                    world.players[player_index].position.x - 1,
                    world.players[player_index].position.y,
                ),
                _ => continue,
            };
            if world.map.is_walkable(&next_pos, next_pos) {
                return Some(direction);
            }
        }
        None
    }
}

fn path_to_action(current: Position, path: &[Position]) -> Option<DirectedAction> {
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

fn use_direction(from: Position, to: Position) -> DirectedAction {
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

fn flee_direction(
    world: &WorldState,
    enemy_pos: Position,
    player_index: usize,
) -> Option<DirectedAction> {
    // Move away from enemy - choose direction that maximizes distance
    // Only consider walkable positions
    let mut best_action = None;
    let player = &world.players[player_index];
    let player_pos = player.position;
    let mut best_distance = player_pos.distance(&enemy_pos);

    let actions = [
        (DirectedAction::MoveNorth, Position::new(player_pos.x, player_pos.y - 1)),
        (DirectedAction::MoveEast, Position::new(player_pos.x + 1, player_pos.y)),
        (DirectedAction::MoveSouth, Position::new(player_pos.x, player_pos.y + 1)),
        (DirectedAction::MoveWest, Position::new(player_pos.x - 1, player_pos.y)),
    ];

    for (action, new_pos) in actions {
        // Only consider walkable positions
        if !world.map.is_walkable(&new_pos, new_pos) {
            continue;
        }

        let dist = new_pos.distance(&enemy_pos);
        if dist > best_distance {
            best_distance = dist;
            best_action = Some(action);
        }
    }

    best_action.or(Some(DirectedAction::None))
}

impl ExecuteGoal for RandomExploreGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let target_pos = self.0;

        debug!("Random exploring to {:?}", target_pos);

        // Try to path to the random position
        world.players[player_index].current_destination = Some(target_pos);
        let path = world.find_path_for_player(player_index, player_pos, target_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
