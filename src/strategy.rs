use crate::goal::Goal;
use crate::pathfinding::AStar;
use crate::swoq_interface::DirectedAction;
use crate::world_state::{Color, WorldState};
use tracing::debug;

pub trait SelectGoal {
    fn try_select(&self, world: &WorldState) -> Option<Goal>;
}

pub struct Planner;

impl Planner {
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn decide_action(world: &mut WorldState) -> (Goal, DirectedAction) {
        // Check if we've reached the current destination
        if let Some(dest) = world.current_destination
            && world.player_pos == dest
        {
            // Reached destination - clear it to select a new goal
            world.current_destination = None;
            world.previous_goal = None;
        }

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ 🧠 PLANNING PHASE - Selecting goal                         ");
        println!("└────────────────────────────────────────────────────────────┘");

        let goal = Self::select_goal(world);

        // Check if goal has changed - if not, keep same destination to avoid oscillation
        let goal_changed = world.previous_goal.as_ref() != Some(&goal);
        if goal_changed {
            world.current_destination = None;
            world.previous_goal = Some(goal.clone());
        }

        println!(
            "  Goal: {:?}, frontier size: {}, player tile: {:?}, dest: {:?}",
            goal,
            world.unexplored_frontier.len(),
            world.map.get(&world.player_pos),
            world.current_destination
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

impl SelectGoal for AttackOrFleeEnemyStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 8 {
            return None;
        }

        let enemy_pos = world.closest_enemy()?;
        let dist = world.player_pos.distance(&enemy_pos);

        // If we have a sword and enemy is close (adjacent or 2 tiles away), attack it
        if world.player_has_sword && dist <= 2 {
            debug!("(have sword, enemy within {} tiles)", dist);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If we don't have sword and enemy is dangerously close, flee
        if dist <= 3 && !world.player_has_sword {
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
            .any(|&enemy_pos| world.player_pos.distance(&enemy_pos) <= 5);

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
        if world.level >= 10 && !world.player_has_sword && !world.swords.is_empty() {
            Some(Goal::PickupSword)
        } else {
            None
        }
    }
}

impl SelectGoal for DropBoulderOnPlateStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        if world.level < 6 || world.player_inventory != crate::swoq_interface::Inventory::Boulder {
            return None;
        }

        debug!("Carrying a boulder, checking for pressure plates");

        // Check if there's a pressure plate we can reach
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plates) = world.pressure_plates.get_positions(color)
                && let Some(&plate_pos) = plates.first()
            {
                // Check if we can reach the plate
                if world.player_pos.is_adjacent(&plate_pos)
                    || AStar::find_path(world, world.player_pos, plate_pos, true).is_some()
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
            if world.has_key(color) {
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
                    if !world.player_pos.is_adjacent(&plate_pos)
                        && AStar::find_path(world, world.player_pos, plate_pos, false).is_none()
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
            if world.has_key(color) {
                let door_pos = world.closest_door_of_color(color)?;
                debug!("We have key for {:?}, door at {:?}", color, door_pos);

                // Check if door is reachable (can path through doors we have keys for)
                if world.player_pos.is_adjacent(&door_pos)
                    || AStar::find_path(world, world.player_pos, door_pos, true).is_some()
                {
                    debug!("(we have the key, door is reachable)");
                    return Some(Goal::OpenDoor(color));
                } else {
                    debug!("Door at {:?} is not reachable", door_pos);
                }
            }
        }

        None
    }
}

impl SelectGoal for GetKeyForDoorStrategy {
    fn try_select(&self, world: &WorldState) -> Option<Goal> {
        for color in world.doors_without_keys() {
            debug!("Checking door without key: {:?}", color);

            // If we know where the key is and can reach it, go get it
            debug!(
                "Checking if we know key location for {:?}: {}",
                color,
                world.knows_key_location(color)
            );
            if world.knows_key_location(color) {
                if let Some(key_pos) = world.closest_key(color) {
                    debug!("Closest key for {:?} is at {:?}", color, key_pos);
                    // Use can_open_doors=true to allow using keys we already have
                    // Use avoid_keys=true to not pick up other keys along the way
                    if AStar::find_path(world, world.player_pos, key_pos, true).is_some() {
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
        let exit_pos = world.exit_pos?;

        // Check if we're carrying a boulder - must drop it before exiting
        if world.player_inventory == crate::swoq_interface::Inventory::Boulder {
            debug!("Need to drop boulder before reaching exit");
            return Some(Goal::DropBoulder);
        }

        // Check if we can actually path to the exit
        if AStar::find_path(world, world.player_pos, exit_pos, true).is_some() {
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
            || world.boulder_info.is_empty()
            || world.player_inventory != crate::swoq_interface::Inventory::None
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
        let mut nearest_boulder: Option<crate::world_state::Pos> = None;
        let mut nearest_distance = i32::MAX;

        for boulder_pos in world.boulder_info.get_all_positions() {
            let dist = world.player_pos.distance(&boulder_pos);
            if dist < nearest_distance {
                // Check if we can reach an adjacent position to pick it up
                let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                    world.is_walkable(&adj, true, true)
                        && AStar::find_path(world, world.player_pos, adj, true).is_some()
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
            || world.boulder_info.is_empty()
            || world.player_inventory != crate::swoq_interface::Inventory::None
        {
            return None;
        }

        debug!(
            "Checking {} boulders for unexplored ones (frontier size: {})",
            world.boulder_info.len(),
            world.unexplored_frontier.len()
        );

        // Check if any boulder is unexplored and reachable
        for boulder_pos in world.boulder_info.get_original_boulders() {
            // Is the boulder unexplored (not moved by us)?
            if !world.boulder_info.has_moved(&boulder_pos) {
                debug!("  Boulder at {:?} is unexplored", boulder_pos);

                // Check if we can reach an adjacent position to pick it up
                let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                    world.is_walkable(&adj, true, true)
                        && AStar::find_path(world, world.player_pos, adj, true).is_some()
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
        if !world.unexplored_frontier.is_empty() {
            return None;
        }

        debug!("Area fully explored, checking for pressure plates to step on");
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plates) = world.pressure_plates.get_positions(color) {
                for &plate_pos in plates {
                    // Check if we can reach the plate
                    if world.player_pos.is_adjacent(&plate_pos)
                        || AStar::find_path(world, world.player_pos, plate_pos, false).is_some()
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
            world.player_has_sword,
            world.unexplored_frontier.is_empty(),
            !world.enemies.is_empty(),
            world.enemies.get_positions().len()
        );

        if !world.player_has_sword
            || !world.unexplored_frontier.is_empty()
            || world.enemies.is_empty()
        {
            return None;
        }

        debug!("Maze fully explored, have sword, hunting enemy (may drop key)");

        // Find the closest enemy
        if let Some(enemy_pos) = world.closest_enemy() {
            debug!("Hunting enemy at {:?}", enemy_pos);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        None
    }
}
