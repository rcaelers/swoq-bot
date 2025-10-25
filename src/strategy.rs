use tracing::debug;

use crate::goal::Goal;
use crate::swoq_interface::DirectedAction;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

pub trait SelectGoal {
    fn try_select(&self, world: &WorldState) -> Option<Goal>;
}

pub struct Planner;

impl Planner {
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn decide_action(world: &mut WorldState) -> (Goal, DirectedAction) {
        // Check if we've reached the current destination
        if let Some(dest) = world.player_mut().current_destination
            && world.player().position == dest
        {
            // Reached destination - clear it to select a new goal
            world.player_mut().current_destination = None;
            world.player_mut().previous_goal = None;
        }

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ 🧠 PLANNING PHASE - Selecting goal                         ");
        println!("└────────────────────────────────────────────────────────────┘");

        let goal = Self::select_goal(world);

        // Check if goal has changed - if not, keep same destination to avoid oscillation
        let goal_changed = world.player_mut().previous_goal.as_ref() != Some(&goal);
        if goal_changed {
            world.player_mut().current_destination = None;
            world.player_mut().previous_goal = Some(goal.clone());
        }

        let frontier_size = world.player().unexplored_frontier.len();
        let player_pos = world.player().position;
        let player_tile = world.map.get(&player_pos);
        let current_dest = world.player().current_destination;

        println!(
            "  Goal: {:?}, frontier size: {}, player tile: {:?}, dest: {:?}",
            goal, frontier_size, player_tile, current_dest
        );

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ ⚡ EXECUTING ACTION - Planning action for goal              ");
        println!("└────────────────────────────────────────────────────────────┘");

        let action = goal.execute(world).unwrap_or(DirectedAction::None);
        (goal, action)
    }

    #[tracing::instrument(level = "debug", skip(world))]
    pub fn select_goal(world: &WorldState) -> Goal {
        let strategies: &[&dyn SelectGoal] = &[
            &AttackOrFleeEnemyStrategy,
            &PickupHealthStrategy,
            &PickupSwordStrategy,
            &DropBoulderOnPlateStrategy,
            &UsePressurePlateForDoorStrategy,
            &OpenDoorWithKeyStrategy,
            &GetKeyForDoorStrategy,
            &ReachExitStrategy,
            &FetchBoulderForPlateStrategy,
            &MoveUnexploredBoulderStrategy,
            &FallbackPressurePlateStrategy,
            &HuntEnemyWithSwordStrategy,
            &RandomExploreStrategy,
        ];

        for strategy in strategies {
            if let Some(goal) = strategy.try_select(world) {
                debug!("Selected goal: {:?}", goal);
                return goal;
            }
        }

        let goal = Goal::Explore;
        debug!("Selected goal: {:?}", goal);
        goal
    }
}

pub struct AttackOrFleeEnemyStrategy;
pub struct PickupHealthStrategy;
pub struct PickupSwordStrategy;
pub struct DropBoulderOnPlateStrategy;
pub struct UsePressurePlateForDoorStrategy;
pub struct OpenDoorWithKeyStrategy;
pub struct GetKeyForDoorStrategy;
pub struct ReachExitStrategy;
pub struct FetchBoulderForPlateStrategy;
pub struct MoveUnexploredBoulderStrategy;
pub struct FallbackPressurePlateStrategy;
pub struct HuntEnemyWithSwordStrategy;
pub struct RandomExploreStrategy;

impl SelectGoal for AttackOrFleeEnemyStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 8 {
            return None;
        }

        let enemy_pos = world.closest_enemy(world.player())?;
        let dist = world.player().position.distance(&enemy_pos);

        // If we have a sword and enemy is close (adjacent or 2 tiles away), attack it
        if world.player().has_sword && dist <= 2 {
            debug!("(have sword, enemy within {} tiles)", dist);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If we don't have sword and enemy is dangerously close, flee
        if dist <= 3 && !world.player().has_sword {
            return Some(Goal::AvoidEnemy(enemy_pos));
        }

        None
    }
}

impl SelectGoal for PickupHealthStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 10 || world.health.is_empty() {
            return None;
        }

        // Check if any enemy is close (within 5 tiles)
        let enemy_nearby = world
            .enemies
            .get_positions()
            .iter()
            .any(|&enemy_pos| world.player().position.distance(&enemy_pos) <= 5);

        if !enemy_nearby {
            debug!("(health found, no enemies nearby)");
            Some(Goal::PickupHealth)
        } else {
            debug!("Health found but enemies nearby (within 5 tiles), skipping for now");
            None
        }
    }
}

impl SelectGoal for PickupSwordStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level >= 10 && !world.player().has_sword && !world.swords.is_empty() {
            Some(Goal::PickupSword)
        } else {
            None
        }
    }
}

impl SelectGoal for DropBoulderOnPlateStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 6 || world.player().inventory != crate::swoq_interface::Inventory::Boulder
        {
            return None;
        }

        debug!("Carrying a boulder, checking for pressure plates");

        // Check if there's a pressure plate we can reach
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plates) = world.pressure_plates.get_positions(color)
                && let Some(&plate_pos) = plates.first()
            {
                // Check if we can reach the plate
                if world.player().position.is_adjacent(&plate_pos)
                    || world
                        .map
                        .find_path(world.player().position, plate_pos)
                        .is_some()
                {
                    debug!("Found reachable {:?} pressure plate at {:?}", color, plate_pos);
                    return Some(Goal::DropBoulderOnPlate(color, plate_pos));
                }
            }
        }

        // No reachable pressure plate, just drop it
        debug!("No reachable pressure plate, need to drop boulder");
        Some(Goal::DropBoulder)
    }
}

impl SelectGoal for UsePressurePlateForDoorStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        // Check if we can use pressure plates to open doors (prefer keys over plates)
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Skip if we have a key for this color
            if world.has_key(world.player(), color) {
                continue;
            }

            let door_positions = world.doors.get_positions(color)?;
            if door_positions.is_empty() {
                continue;
            }

            // Get pressure plates for this color
            if let Some(plates) = world.pressure_plates.get_positions(color) {
                // Check each pressure plate to see if we can stand on it
                for &plate_pos in plates {
                    // Can we reach the plate?
                    if !world.player().position.is_adjacent(&plate_pos)
                        && world
                            .map
                            .find_path(world.player().position, plate_pos)
                            .is_none()
                    {
                        continue;
                    }

                    // Is there a door of the same color adjacent to this plate?
                    for &neighbor in &plate_pos.neighbors() {
                        if door_positions.contains(&neighbor) {
                            debug!(
                                "Found {:?} pressure plate at {:?} adjacent to door at {:?}",
                                color, plate_pos, neighbor
                            );
                            return Some(Goal::StepOnPressurePlate(color, plate_pos));
                        }
                    }
                }
            }
        }

        None
    }
}

impl SelectGoal for OpenDoorWithKeyStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        for &color in world.doors.colors() {
            if world.has_key(world.player(), color) {
                let door_positions = world.doors.get_positions(color)?;

                // Check if any door of this color has a reachable empty neighbor
                for &door_pos in door_positions {
                    debug!("Checking door {:?} at {:?}", color, door_pos);

                    // Check if any neighbor of the door is reachable
                    let has_reachable_neighbor = door_pos.neighbors().iter().any(|&neighbor| {
                        // Only consider empty tiles (or player position)
                        if neighbor != world.player().position
                            && !matches!(
                                world.map.get(&neighbor),
                                Some(crate::swoq_interface::Tile::Empty)
                            )
                        {
                            return false;
                        }

                        // Check if player is already at this neighbor or can path to it
                        world.player().position == neighbor
                            || world
                                .map
                                .find_path(world.player().position, neighbor)
                                .is_some()
                    });

                    if has_reachable_neighbor {
                        debug!("(we have the key, door has reachable empty neighbor)");
                        return Some(Goal::OpenDoor(color));
                    } else {
                        debug!("Door at {:?} has no reachable empty neighbors", door_pos);
                    }
                }
            }
        }

        None
    }
}

impl SelectGoal for GetKeyForDoorStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        for color in world.doors_without_keys(world.player()) {
            debug!("Checking door without key: {:?}", color);

            // If we know where the key is and can reach it, go get it
            debug!(
                "Checking if we know key location for {:?}: {}",
                color,
                world.knows_key_location(color)
            );
            if world.knows_key_location(color) {
                if let Some(key_pos) = world.closest_key(world.player(), color) {
                    debug!("Closest key for {:?} is at {:?}", color, key_pos);
                    // Use can_open_doors=true to allow using keys we already have
                    // Use avoid_keys=true to not pick up other keys along the way
                    if world
                        .map
                        .find_path(world.player().position, key_pos)
                        .is_some()
                    {
                        debug!("(key is reachable)");
                        return Some(Goal::GetKey(color));
                    } else {
                        debug!("Key at {:?} is not reachable", key_pos);
                    }
                } else {
                    debug!("No keys found for {:?}!", color);
                }
            }
        }

        None
    }
}

impl SelectGoal for ReachExitStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        let exit_pos = world.exit_position?;

        // Check if we're carrying a boulder - must drop it before exiting
        if world.player().inventory == crate::swoq_interface::Inventory::Boulder {
            debug!("Need to drop boulder before reaching exit");
            return Some(Goal::DropBoulder);
        }

        // Check if we can actually path to the exit
        if world
            .map
            .find_path(world.player().position, exit_pos)
            .is_some()
        {
            Some(Goal::ReachExit)
        } else {
            debug!("Exit at {:?} is not reachable, continuing exploration", exit_pos);
            None
        }
    }
}

impl SelectGoal for FetchBoulderForPlateStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 6
            || world.boulders.is_empty()
            || world.player().inventory != crate::swoq_interface::Inventory::None
        {
            return None;
        }

        // First priority: if there's a pressure plate, fetch a boulder for it
        let has_pressure_plates = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .any(|&color| world.pressure_plates.has_color(color));

        if !has_pressure_plates {
            return None;
        }

        debug!("Found pressure plates, looking for nearest boulder");

        // Find nearest reachable boulder
        let mut nearest_boulder: Option<Position> = None;
        let mut nearest_distance = i32::MAX;

        for boulder_pos in world.boulders.get_all_positions() {
            let dist = world.player().position.distance(&boulder_pos);
            if dist < nearest_distance {
                // Check if we can reach an adjacent position to pick it up
                let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                    world.map.is_walkable(&adj, adj)
                        && world.map.find_path(world.player().position, adj).is_some()
                });

                if can_reach {
                    nearest_boulder = Some(boulder_pos);
                    nearest_distance = dist;
                }
            }
        }

        if let Some(boulder_pos) = nearest_boulder {
            debug!("Fetching boulder at {:?} for pressure plate", boulder_pos);
            return Some(Goal::FetchBoulder(boulder_pos));
        }

        None
    }
}

impl SelectGoal for MoveUnexploredBoulderStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 6
            || world.boulders.is_empty()
            || world.player().inventory != crate::swoq_interface::Inventory::None
        {
            return None;
        }

        debug!(
            "Checking {} boulders for unexplored ones (frontier size: {})",
            world.boulders.len(),
            world.player().unexplored_frontier.len()
        );

        // Check if any boulder is unexplored and reachable
        for boulder_pos in world.boulders.get_original_boulders() {
            // Is the boulder unexplored (not moved by us)?
            if !world.boulders.has_moved(&boulder_pos) {
                debug!("  Boulder at {:?} is unexplored", boulder_pos);

                // Check if we can reach an adjacent position to pick it up
                let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                    world.map.is_walkable(&adj, adj)
                        && world.map.find_path(world.player().position, adj).is_some()
                });

                if can_reach {
                    debug!("  Boulder at {:?} is reachable - will move it", boulder_pos);
                    return Some(Goal::FetchBoulder(boulder_pos));
                } else {
                    debug!("  Boulder at {:?} is not reachable yet", boulder_pos);
                }
            }
        }
        debug!("No reachable unexplored boulders found");

        None
    }
}

impl SelectGoal for FallbackPressurePlateStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        // If nothing else to do and area is fully explored, step on any reachable pressure plate
        if !world.player().unexplored_frontier.is_empty() {
            return None;
        }

        debug!("Area fully explored, checking for pressure plates to step on");
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plates) = world.pressure_plates.get_positions(color) {
                for &plate_pos in plates {
                    // Check if we can reach the plate
                    if world.player().position.is_adjacent(&plate_pos)
                        || world
                            .map
                            .find_path(world.player().position, plate_pos)
                            .is_some()
                    {
                        debug!(
                            "Found reachable {:?} pressure plate at {:?} as fallback",
                            color, plate_pos
                        );
                        return Some(Goal::StepOnPressurePlate(color, plate_pos));
                    }
                }
            }
        }

        None
    }
}

impl SelectGoal for HuntEnemyWithSwordStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        // Only hunt enemies when:
        // 1. We have a sword
        // 2. The entire maze is explored (frontier is empty)
        // 3. There are enemies present
        debug!(
            "HuntEnemyWithSwordStrategy check: has_sword={}, frontier_empty={}, enemies_present={} (count={})",
            world.player().has_sword,
            world.player().unexplored_frontier.is_empty(),
            !world.enemies.is_empty(),
            world.enemies.get_positions().len()
        );

        if !world.player().has_sword
            || !world.player().unexplored_frontier.is_empty()
            || world.enemies.is_empty()
        {
            return None;
        }

        debug!("Maze fully explored, have sword, hunting enemy (may drop key)");

        // Find the closest enemy
        if let Some(enemy_pos) = world.closest_enemy(world.player()) {
            debug!("Hunting enemy at {:?}", enemy_pos);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        None
    }
}

impl SelectGoal for RandomExploreStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        // Only use random exploration when:
        // 1. The frontier is empty (nothing new to explore)
        // 2. We're not doing anything else
        if !world.player().unexplored_frontier.is_empty() {
            return None;
        }

        // If we already have a RandomExplore goal and destination, keep it
        if let Some(Goal::RandomExplore(_)) = &world.player().previous_goal
            && world.player().current_destination.is_some()
        {
            debug!("RandomExploreStrategy: Continuing with existing destination");
            return world.player().previous_goal.clone();
        }

        debug!("RandomExploreStrategy: Frontier empty, selecting random reachable position");

        // Collect all empty positions that we've seen
        let empty_positions: Vec<Position> = world
            .map
            .iter()
            .filter_map(|(pos, tile)| {
                if matches!(tile, crate::swoq_interface::Tile::Empty)
                    && world.player().position.distance(pos) > 5
                {
                    Some(*pos)
                } else {
                    None
                }
            })
            .collect();

        if empty_positions.is_empty() {
            debug!("RandomExploreStrategy: No empty positions found");
            return None;
        }

        // Try random positions until we find a reachable one (max 10 attempts)
        let mut seed = world.tick as usize;
        for _ in 0..10 {
            let index = seed % empty_positions.len();
            let target = empty_positions[index];

            // Check if reachable
            if world
                .map
                .find_path(world.player().position, target)
                .is_some()
            {
                debug!("RandomExploreStrategy: Selected reachable position {:?}", target);
                return Some(Goal::RandomExplore(target));
            }

            // Try next position
            seed = seed.wrapping_add(1);
        }

        debug!("RandomExploreStrategy: No reachable position found after 10 attempts");
        None
    }
}
