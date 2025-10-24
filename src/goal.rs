use crate::pathfinding::AStar;
use crate::swoq_interface::DirectedAction;
use crate::world_state::{Color, Pos, WorldState};
use tracing::debug;

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Explore,
    GetKey(Color),
    OpenDoor(Color),
    StepOnPressurePlate(Color, Pos),
    PickupSword,
    PickupHealth,
    AvoidEnemy(Pos),
    KillEnemy(Pos),
    FetchBoulder(Pos),
    DropBoulder,
    DropBoulderOnPlate(Color, Pos),
    ReachExit,
}

impl Goal {
    pub fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        match self {
            Goal::Explore => ExploreGoal.execute(world),
            Goal::GetKey(color) => GetKeyGoal(*color).execute(world),
            Goal::OpenDoor(color) => OpenDoorGoal(*color).execute(world),
            Goal::StepOnPressurePlate(_color, pos) => {
                StepOnPressurePlateGoal(*pos).execute(world)
            }
            Goal::PickupSword => PickupSwordGoal.execute(world),
            Goal::PickupHealth => PickupHealthGoal.execute(world),
            Goal::ReachExit => ReachExitGoal.execute(world),
            Goal::KillEnemy(pos) => KillEnemyGoal(*pos).execute(world),
            Goal::AvoidEnemy(pos) => AvoidEnemyGoal(*pos).execute(world),
            Goal::FetchBoulder(pos) => FetchBoulderGoal(*pos).execute(world),
            Goal::DropBoulderOnPlate(_color, pos) => {
                DropBoulderOnPlateGoal(*pos).execute(world)
            }
            Goal::DropBoulder => DropBoulderGoal.execute(world),
        }
    }
}

pub trait ExecuteGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction>;
}

struct ExploreGoal;
struct GetKeyGoal(Color);
struct OpenDoorGoal(Color);
struct StepOnPressurePlateGoal(Pos);
struct PickupSwordGoal;
struct PickupHealthGoal;
struct AvoidEnemyGoal(Pos);
struct KillEnemyGoal(Pos);
struct FetchBoulderGoal(Pos);
struct DropBoulderGoal;
struct DropBoulderOnPlateGoal(Pos);
struct ReachExitGoal;

impl ExecuteGoal for ExploreGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        // If we have a destination, try to continue using it
        if let Some(dest) = world.current_destination {
            if let Some(path) = AStar::find_path(world, world.player_pos, dest, false) {
                debug!("Continuing to existing destination {:?}, path length={}", dest, path.len());
                world.current_path = Some(path.clone());
                return path_to_action(world.player_pos, &path);
            } else {
                // Destination became unreachable
                debug!("Destination {:?} is now unreachable, finding new one", dest);
                world.current_destination = None;
                world.current_path = None;
            }
        }

        // Only search for new destination if we don't have one
        debug!(
            "Searching for new frontier destination from {} tiles",
            world.sorted_unexplored().len()
        );
        let mut attempts = 0;
        for (i, target) in world.sorted_unexplored().iter().enumerate() {
            if i < 5 {
                debug!(
                    "  Trying frontier #{}: {:?}, distance={}",
                    i,
                    target,
                    world.player_pos.distance(target)
                );
            }
            attempts += 1;
            if let Some(path) = AStar::find_path(world, world.player_pos, *target, false) {
                debug!(
                    "New frontier destination: {:?}, path length={} (tried {} tiles)",
                    target,
                    path.len(),
                    attempts
                );
                world.current_destination = Some(*target);
                world.current_path = Some(path.clone());
                return path_to_action(world.player_pos, &path);
            }
        }
        debug!(
            "No reachable frontier tiles found out of {} candidates (tried {} tiles)",
            world.sorted_unexplored().len(),
            attempts
        );
        world.current_destination = None;
        world.current_path = None;

        None
    }
}

impl ExecuteGoal for GetKeyGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let key_pos = world.key_positions.get(&self.0)?;
        world.current_destination = Some(*key_pos);
        let path = AStar::find_path(world, world.player_pos, *key_pos, false)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for OpenDoorGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let door_positions = world.door_positions.get(&self.0)?;
        let door_pos = *door_positions.first()?;

        // OpenDoor is only for keys - if we don't have a key, this shouldn't be selected
        if !world.has_key(self.0) {
            debug!("OpenDoor goal but no {:?} key!", self.0);
            return None;
        }

        // If adjacent to door, use key on it
        if world.player_pos.is_adjacent(&door_pos) {
            return Some(use_direction(world.player_pos, door_pos));
        }

        // Navigate to door (can open doors since we have keys)
        world.current_destination = Some(door_pos);
        let path = AStar::find_path(world, world.player_pos, door_pos, true)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for StepOnPressurePlateGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let plate_pos = self.0;

        // Navigate to and step on the pressure plate
        // Once on the plate, the door will be open (empty tile) so we can walk through
        if world.player_pos == plate_pos {
            // Already on the plate - the door should be open now, just explore
            debug!("Already on pressure plate at {:?}, doors should be open", plate_pos);
            None // Let exploration handle moving through the now-open door
        } else if world.player_pos.is_adjacent(&plate_pos) {
            // Move onto the pressure plate
            debug!("Moving onto pressure plate at {:?}", plate_pos);
            let dx = plate_pos.x - world.player_pos.x;
            let dy = plate_pos.y - world.player_pos.y;
            Some(if dy < 0 {
                DirectedAction::MoveNorth
            } else if dy > 0 {
                DirectedAction::MoveSouth
            } else if dx > 0 {
                DirectedAction::MoveEast
            } else {
                DirectedAction::MoveWest
            })
        } else {
            // Navigate to the pressure plate
            debug!("Navigating to pressure plate at {:?}", plate_pos);
            world.current_destination = Some(plate_pos);
            let path = AStar::find_path(world, world.player_pos, plate_pos, false)?;
            world.current_path = Some(path.clone());
            path_to_action(world.player_pos, &path)
        }
    }
}

impl ExecuteGoal for PickupSwordGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let sword_pos = *world.sword_positions.first()?;
        world.current_destination = Some(sword_pos);
        let path = AStar::find_path(world, world.player_pos, sword_pos, false)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let health_pos = world.closest_health()?;
        world.current_destination = Some(health_pos);
        let path = AStar::find_path(world, world.player_pos, health_pos, false)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for ReachExitGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let exit_pos = world.exit_pos?;
        world.current_destination = Some(exit_pos);
        let path = AStar::find_path(world, world.player_pos, exit_pos, true)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for KillEnemyGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let enemy_pos = self.0;

        // If adjacent, attack
        if world.player_pos.is_adjacent(&enemy_pos) {
            return Some(use_direction(world.player_pos, enemy_pos));
        }

        // Move adjacent to enemy
        for adjacent in enemy_pos.neighbors() {
            if world.is_walkable(&adjacent, false, true)
                && let Some(path) = AStar::find_path(world, world.player_pos, adjacent, false)
            {
                world.current_path = Some(path.clone());
                return path_to_action(world.player_pos, &path);
            }
        }
        None
    }
}

impl ExecuteGoal for AvoidEnemyGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        flee_direction(world, self.0)
    }
}

impl ExecuteGoal for FetchBoulderGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let boulder_pos = self.0;

        // If we're already adjacent, pick up the boulder
        if world.player_pos.is_adjacent(&boulder_pos) {
            debug!("Picking up boulder at {:?}", boulder_pos);
            return Some(use_direction(world.player_pos, boulder_pos));
        }

        // Navigate to an adjacent walkable position next to the boulder
        for adjacent in boulder_pos.neighbors() {
            if world.is_walkable(&adjacent, true, true)
                && let Some(path) = AStar::find_path(world, world.player_pos, adjacent, true)
            {
                debug!("Moving to adjacent position {:?} to reach boulder", adjacent);
                world.current_destination = Some(adjacent);
                world.current_path = Some(path.clone());
                return path_to_action(world.player_pos, &path);
            }
        }
        debug!("No walkable position adjacent to boulder at {:?}", boulder_pos);
        None
    }
}

impl ExecuteGoal for DropBoulderOnPlateGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let plate_pos = self.0;

        // If we're adjacent to the pressure plate, drop the boulder on it
        if world.player_pos.is_adjacent(&plate_pos) {
            debug!("Dropping boulder on pressure plate at {:?}", plate_pos);
            return Some(use_direction(world.player_pos, plate_pos));
        }

        // Navigate to the pressure plate
        world.current_destination = Some(plate_pos);
        let path = AStar::find_path(world, world.player_pos, plate_pos, true)?;
        world.current_path = Some(path.clone());
        path_to_action(world.player_pos, &path)
    }
}

impl ExecuteGoal for DropBoulderGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        // Find a safe place to drop the boulder (empty adjacent tile)
        for neighbor in world.player_pos.neighbors() {
            if matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                && neighbor.x >= 0
                && neighbor.x < world.map_width
                && neighbor.y >= 0
                && neighbor.y < world.map_height
            {
                debug!("Dropping boulder at {:?}", neighbor);
                return Some(use_direction(world.player_pos, neighbor));
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
                DirectedAction::MoveNorth => Pos::new(world.player_pos.x, world.player_pos.y - 1),
                DirectedAction::MoveEast => Pos::new(world.player_pos.x + 1, world.player_pos.y),
                DirectedAction::MoveSouth => Pos::new(world.player_pos.x, world.player_pos.y + 1),
                DirectedAction::MoveWest => Pos::new(world.player_pos.x - 1, world.player_pos.y),
                _ => continue,
            };
            if world.is_walkable(&next_pos, true, true) {
                return Some(direction);
            }
        }
        None
    }
}

fn path_to_action(current: Pos, path: &[Pos]) -> Option<DirectedAction> {
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

fn use_direction(from: Pos, to: Pos) -> DirectedAction {
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

fn flee_direction(world: &WorldState, enemy_pos: Pos) -> Option<DirectedAction> {
    // Move away from enemy - choose direction that maximizes distance
    // Only consider walkable positions
    let mut best_action = None;
    let mut best_distance = world.player_pos.distance(&enemy_pos);

    let actions = [
        (DirectedAction::MoveNorth, Pos::new(world.player_pos.x, world.player_pos.y - 1)),
        (DirectedAction::MoveEast, Pos::new(world.player_pos.x + 1, world.player_pos.y)),
        (DirectedAction::MoveSouth, Pos::new(world.player_pos.x, world.player_pos.y + 1)),
        (DirectedAction::MoveWest, Pos::new(world.player_pos.x - 1, world.player_pos.y)),
    ];

    for (action, new_pos) in actions {
        // Only consider walkable positions
        if !world.is_walkable(&new_pos, true, true) {
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
