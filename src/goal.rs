use tracing::debug;

use crate::pathfinding::AStar;
use crate::swoq_interface::DirectedAction;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Explore,
    GetKey(Color),
    OpenDoor(Color),
    StepOnPressurePlate(Color, Position),
    PickupSword,
    PickupHealth,
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
    pub fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        match self {
            Goal::Explore => ExploreGoal.execute(world),
            Goal::GetKey(color) => GetKeyGoal(*color).execute(world),
            Goal::OpenDoor(color) => OpenDoorGoal(*color).execute(world),
            Goal::StepOnPressurePlate(_color, pos) => StepOnPressurePlateGoal(*pos).execute(world),
            Goal::PickupSword => PickupSwordGoal.execute(world),
            Goal::PickupHealth => PickupHealthGoal.execute(world),
            Goal::ReachExit => ReachExitGoal.execute(world),
            Goal::KillEnemy(pos) => KillEnemyGoal(*pos).execute(world),
            Goal::AvoidEnemy(pos) => AvoidEnemyGoal(*pos).execute(world),
            Goal::FetchBoulder(pos) => FetchBoulderGoal(*pos).execute(world),
            Goal::DropBoulderOnPlate(_color, pos) => DropBoulderOnPlateGoal(*pos).execute(world),
            Goal::DropBoulder => DropBoulderGoal.execute(world),
            Goal::RandomExplore(pos) => RandomExploreGoal(*pos).execute(world),
        }
    }
}

pub trait ExecuteGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction>;
}

struct ExploreGoal;
struct GetKeyGoal(Color);
struct OpenDoorGoal(Color);
struct StepOnPressurePlateGoal(Position);
struct PickupSwordGoal;
struct PickupHealthGoal;
struct AvoidEnemyGoal(Position);
struct KillEnemyGoal(Position);
struct FetchBoulderGoal(Position);
struct DropBoulderGoal;
struct DropBoulderOnPlateGoal(Position);
struct ReachExitGoal;
struct RandomExploreGoal(Position);

impl ExecuteGoal for ExploreGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        // If we have a destination, try to continue using it
        let player_pos = world.player().position;
        if let Some(dest) = world.player_mut().current_destination {
            if let Some(path) = AStar::find_path(&world.map, player_pos, dest, |pos, goal| {
                world.is_walkable(world.player(), pos, goal, false)
            }) {
                debug!("Continuing to existing destination {:?}, path length={}", dest, path.len());
                world.player_mut().current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            } else {
                // Destination became unreachable
                debug!("Destination {:?} is now unreachable, finding new one", dest);
                world.player_mut().current_destination = None;
                world.player_mut().current_path = None;
            }
        }

        // Only search for new frontier destination if we don't have one
        let sorted_frontier = world.player().sorted_unexplored();
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
            if let Some(path) = AStar::find_path(&world.map, player_pos, *target, |pos, goal| {
                world.is_walkable(world.player(), pos, goal, false)
            }) {
                debug!(
                    "New frontier destination: {:?}, path length={} (tried {} tiles)",
                    target,
                    path.len(),
                    attempts
                );
                world.player_mut().current_destination = Some(*target);
                world.player_mut().current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!(
            "No reachable frontier tiles found out of {} candidates (tried {} tiles)",
            sorted_frontier.len(),
            attempts
        );
        world.player_mut().current_destination = None;
        world.player_mut().current_path = None;

        None
    }
}

impl ExecuteGoal for GetKeyGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let key_pos = world.closest_key(world.player(), self.0)?;
        world.player_mut().current_destination = Some(key_pos);
        let path = AStar::find_path(&world.map, player_pos, key_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, false)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for OpenDoorGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let door_positions = world.doors.get_positions(self.0)?;

        // Find the closest reachable door (not just by distance)
        let mut closest_door: Option<(Position, usize)> = None;
        for &door_pos in door_positions {
            if let Some(path) = AStar::find_path(&world.map, player_pos, door_pos, |pos, goal| {
                world.is_walkable(world.player(), pos, goal, false)
            }) {
                let path_len = path.len();
                if closest_door.is_none() || path_len < closest_door.unwrap().1 {
                    closest_door = Some((door_pos, path_len));
                }
            }
        }

        let door_pos = closest_door.map(|(pos, _)| pos)?;

        // OpenDoor is only for keys - if we don't have a key, this shouldn't be selected
        if !world.has_key(world.player(), self.0) {
            debug!("OpenDoor goal but no {:?} key!", self.0);
            return None;
        }

        // If adjacent to door, use key on it
        if player_pos.is_adjacent(&door_pos) {
            return Some(use_direction(player_pos, door_pos));
        }

        // Navigate to door (cannot open doors in pathfinding - door is destination only)
        world.player_mut().current_destination = Some(door_pos);
        let path = AStar::find_path(&world.map, player_pos, door_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, false)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for StepOnPressurePlateGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let plate_pos = self.0;

        // Navigate to and step on the pressure plate
        // Once on the plate, the door will be open (empty tile) so we can walk through
        if player_pos == plate_pos {
            // Already on the plate - the door should be open now, just explore
            debug!("Already on pressure plate at {:?}, doors should be open", plate_pos);
            None // Let exploration handle moving through the now-open door
        } else if player_pos.is_adjacent(&plate_pos) {
            // Move onto the pressure plate
            debug!("Moving onto pressure plate at {:?}", plate_pos);
            let dx = plate_pos.x - player_pos.x;
            let dy = plate_pos.y - player_pos.y;
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
            world.player_mut().current_destination = Some(plate_pos);
            let path = AStar::find_path(&world.map, player_pos, plate_pos, |pos, goal| {
                world.is_walkable(world.player(), pos, goal, false)
            })?;
            world.player_mut().current_path = Some(path.clone());
            path_to_action(player_pos, &path)
        }
    }
}

impl ExecuteGoal for PickupSwordGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let sword_pos = *world.swords.get_positions().first()?;
        world.player_mut().current_destination = Some(sword_pos);
        let path = AStar::find_path(&world.map, player_pos, sword_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, false)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let health_pos = world.closest_health(world.player())?;
        world.player_mut().current_destination = Some(health_pos);
        let path = AStar::find_path(&world.map, player_pos, health_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, false)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for ReachExitGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let exit_pos = world.exit_position?;
        world.player_mut().current_destination = Some(exit_pos);
        let path = AStar::find_path(&world.map, player_pos, exit_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, true)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for KillEnemyGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let enemy_pos = self.0;

        // If adjacent, attack
        if player_pos.is_adjacent(&enemy_pos) {
            return Some(use_direction(player_pos, enemy_pos));
        }

        // Move adjacent to enemy
        for adjacent in enemy_pos.neighbors() {
            if world.is_walkable(world.player(), &adjacent, adjacent, false)
                && let Some(path) =
                    AStar::find_path(&world.map, player_pos, adjacent, |pos, goal| {
                        world.is_walkable(world.player(), pos, goal, false)
                    })
            {
                world.player_mut().current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
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
        let player_pos = world.player().position;
        let boulder_pos = self.0;

        // If we're already adjacent, pick up the boulder
        if player_pos.is_adjacent(&boulder_pos) {
            debug!("Picking up boulder at {:?}", boulder_pos);
            return Some(use_direction(player_pos, boulder_pos));
        }

        // Navigate to an adjacent walkable position next to the boulder
        for adjacent in boulder_pos.neighbors() {
            if world.is_walkable(world.player(), &adjacent, adjacent, true)
                && let Some(path) =
                    AStar::find_path(&world.map, player_pos, adjacent, |pos, goal| {
                        world.is_walkable(world.player(), pos, goal, true)
                    })
            {
                debug!("Moving to adjacent position {:?} to reach boulder", adjacent);
                world.player_mut().current_destination = Some(adjacent);
                world.player_mut().current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!("No walkable position adjacent to boulder at {:?}", boulder_pos);
        None
    }
}

impl ExecuteGoal for DropBoulderOnPlateGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let plate_pos = self.0;

        // If we're adjacent to the pressure plate, drop the boulder on it
        if player_pos.is_adjacent(&plate_pos) {
            debug!("Dropping boulder on pressure plate at {:?}", plate_pos);
            return Some(use_direction(player_pos, plate_pos));
        }

        // Navigate to the pressure plate
        world.player_mut().current_destination = Some(plate_pos);
        let path = AStar::find_path(&world.map, player_pos, plate_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, true)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}

impl ExecuteGoal for DropBoulderGoal {
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
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
                DirectedAction::MoveNorth => {
                    Position::new(world.player_mut().position.x, world.player_mut().position.y - 1)
                }
                DirectedAction::MoveEast => {
                    Position::new(world.player_mut().position.x + 1, world.player_mut().position.y)
                }
                DirectedAction::MoveSouth => {
                    Position::new(world.player_mut().position.x, world.player_mut().position.y + 1)
                }
                DirectedAction::MoveWest => {
                    Position::new(world.player_mut().position.x - 1, world.player_mut().position.y)
                }
                _ => continue,
            };
            if world.is_walkable(world.player(), &next_pos, next_pos, true) {
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

fn flee_direction(world: &WorldState, enemy_pos: Position) -> Option<DirectedAction> {
    // Move away from enemy - choose direction that maximizes distance
    // Only consider walkable positions
    let mut best_action = None;
    let player_pos = world.player().position;
    let mut best_distance = player_pos.distance(&enemy_pos);

    let actions = [
        (DirectedAction::MoveNorth, Position::new(player_pos.x, player_pos.y - 1)),
        (DirectedAction::MoveEast, Position::new(player_pos.x + 1, player_pos.y)),
        (DirectedAction::MoveSouth, Position::new(player_pos.x, player_pos.y + 1)),
        (DirectedAction::MoveWest, Position::new(player_pos.x - 1, player_pos.y)),
    ];

    for (action, new_pos) in actions {
        // Only consider walkable positions
        if !world.is_walkable(world.player(), &new_pos, new_pos, true) {
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
    fn execute(&self, world: &mut WorldState) -> Option<DirectedAction> {
        let player_pos = world.player().position;
        let target_pos = self.0;

        debug!("Random exploring to {:?}", target_pos);

        // Try to path to the random position
        world.player_mut().current_destination = Some(target_pos);
        let path = AStar::find_path(&world.map, player_pos, target_pos, |pos, goal| {
            world.is_walkable(world.player(), pos, goal, true)
        })?;
        world.player_mut().current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
