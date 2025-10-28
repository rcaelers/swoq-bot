use tracing::debug;

use crate::goal::Goal;
use crate::swoq_interface::DirectedAction;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrategyType {
    /// Strategy is executed once per player independently
    SinglePlayer,
    /// Strategy is evaluated at the same priority level for cooperative tasks.
    /// Each player is evaluated independently and may get different goals or no goal.
    Coop,
}

pub trait SelectGoal {
    /// Returns the strategy type (single-player or co-op)
    fn strategy_type(&self) -> StrategyType;

    /// Try to select a goal for a specific player (0 or 1)
    /// For SinglePlayer strategies only
    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let _ = (world, player_index);
        None
    }

    /// Try to select goals for all players at once
    /// For Coop strategies only. Returns a Vec with one Option<Goal> per player.
    /// Can return different goals for different players, or None for some/all.
    /// `current_goals` contains the already-assigned goals (None if no goal yet).
    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let _ = (world, current_goals);
        vec![None; world.players.len()]
    }
}

pub struct Planner;

impl Planner {
    #[tracing::instrument(level = "debug", skip(world))]
    pub fn decide_action(world: &mut WorldState) -> Vec<(Goal, DirectedAction)> {
        let num_players = world.players.len();

        // Check if each player has reached their current destination
        for player_index in 0..num_players {
            if let Some(dest) = world.players[player_index].current_destination
                && world.players[player_index].position == dest
            {
                // Reached destination - clear it to select a new goal
                world.players[player_index].current_destination = None;
            }
        }

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ 🧠 PLANNING PHASE - Selecting goals                        ");
        println!("└────────────────────────────────────────────────────────────┘");

        for player_index in 0..num_players {
            world.players[player_index].current_goal = None;
        }

        let goals = Self::select_goal(world);

        // Display selected goals
        for (player_index, goal) in goals.iter().enumerate() {
            if player_index < num_players {
                let frontier_size = world.players[player_index].unexplored_frontier.len();
                let player_pos = world.players[player_index].position;
                let player_tile = world.map.get(&player_pos);
                let current_dest = world.players[player_index].current_destination;

                println!(
                    "  Player {}: {:?}, frontier size: {}, tile: {:?}, dest: {:?}",
                    player_index + 1,
                    goal,
                    frontier_size,
                    player_tile,
                    current_dest
                );
            }
        }

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ ⚡ EXECUTING ACTIONS - Planning actions for goals           ");
        println!("└────────────────────────────────────────────────────────────┘");

        // Execute each goal and collect actions
        let mut results = Vec::new();
        for (player_index, goal) in goals.into_iter().enumerate() {
            if player_index < num_players {
                // Set current goal before execution
                world.players[player_index].current_goal = Some(goal.clone());
                let action = goal
                    .execute_for_player(world, player_index)
                    .unwrap_or(DirectedAction::None);

                // Update previous goal after execution
                world.players[player_index].previous_goal = Some(goal.clone());
                results.push((goal, action));
            }
        }

        results
    }

    #[tracing::instrument(level = "debug", skip(world))]
    pub fn select_goal(world: &WorldState) -> Vec<Goal> {
        let strategies: &[&dyn SelectGoal] = &[
            &CooperativeDoorPassageStrategy,
            &AttackOrFleeEnemyStrategy,
            &PickupHealthStrategy,
            &PickupSwordStrategy,
            &CooperativeDoorPassageStrategySetup,
            &DropBoulderOnPlateStrategy,
            &DropBoulderStrategy,
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

        let num_players = world.players.len();
        let mut selected_goals: Vec<Option<Goal>> = vec![None; num_players];

        // Process strategies in order
        for strategy in strategies {
            match strategy.strategy_type() {
                StrategyType::SinglePlayer => {
                    // Execute once per player independently
                    for (player_index, goal_slot) in selected_goals.iter_mut().enumerate() {
                        if goal_slot.is_none()
                            && let Some(goal) = strategy.try_select(world, player_index)
                        {
                            debug!("Player {} selected goal: {:?}", player_index + 1, goal);
                            *goal_slot = Some(goal);
                        }
                    }
                }
                StrategyType::Coop => {
                    // Execute once for all players
                    // Co-op strategy can return different goals for different players
                    if selected_goals.iter().any(|g| g.is_none()) {
                        let coop_goals = strategy.try_select_coop(world, &selected_goals);
                        for player_index in 0..num_players.min(coop_goals.len()) {
                            if selected_goals[player_index].is_none()
                                && let Some(goal) = &coop_goals[player_index]
                            {
                                debug!(
                                    "Player {} selected co-op goal: {:?}",
                                    player_index + 1,
                                    goal
                                );
                                selected_goals[player_index] = Some(goal.clone());
                            }
                        }
                    }
                }
            }

            // If all players have goals, we're done
            if selected_goals.iter().all(|g| g.is_some()) {
                break;
            }
        }

        // Convert to Vec<Goal>, using Explore as default for any player without a goal
        let goals: Vec<Goal> = selected_goals
            .into_iter()
            .enumerate()
            .map(|(idx, goal)| {
                let g = goal.unwrap_or(Goal::Explore);
                debug!("Final selected goal for player {}: {:?}", idx + 1, g);
                g
            })
            .collect();

        goals
    }
}

pub struct AttackOrFleeEnemyStrategy;
pub struct CooperativeDoorPassageStrategy;
pub struct PickupHealthStrategy;
pub struct PickupSwordStrategy;
pub struct DropBoulderOnPlateStrategy;
pub struct DropBoulderStrategy;
pub struct CooperativeDoorPassageStrategySetup;
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
    fn strategy_type(&self) -> StrategyType {
        StrategyType::SinglePlayer
    }

    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        if world.level < 8 {
            return None;
        }

        let player = &world.players[player_index];
        let enemy_pos = world.closest_enemy(player)?;
        let dist = world.path_distance_to_enemy(player.position, enemy_pos);

        // If we have a sword and enemy is close (adjacent or 2 tiles away), attack it
        if player.has_sword && dist <= 2 {
            debug!("(have sword, enemy within {} tiles)", dist);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If we don't have sword and enemy is dangerously close, flee
        if dist <= 3 && !player.has_sword {
            return Some(Goal::AvoidEnemy(enemy_pos));
        }

        None
    }
}

impl SelectGoal for CooperativeDoorPassageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        _current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        // Only works in 2-player mode
        if world.players.len() != 2 {
            return vec![None; 2];
        }

        // Check if one player is waiting on a pressure plate and the other is passing through the door
        // This is a SAFETY-CRITICAL check that must override all other strategies
        debug!(
            "CooperativeDoorPassageStrategy: Checking cooperation - P1 prev goal: {:?}, P2 prev goal: {:?}",
            world.players[0].previous_goal, world.players[1].previous_goal
        );
        debug!(
            "CooperativeDoorPassageStrategy: P1 pos: {:?}, P2 pos: {:?}",
            world.players[0].position, world.players[1].position
        );

        for player_index in 0..2 {
            // Check both directions: waiting player looking for passing player, OR passing player looking for waiting player

            // Case 1: This player has WaitOnPressurePlate, check if other has PassThroughDoor
            if let Some(Goal::WaitOnPressurePlate(color, plate_pos)) =
                world.players[player_index].previous_goal.as_ref()
            {
                let other_player_index = 1 - player_index;
                let other_player = &world.players[other_player_index];
                let waiting_player = &world.players[player_index];

                debug!(
                    "CooperativeDoorPassageStrategy: P{} has WaitOnPressurePlate({:?}, {:?})",
                    player_index + 1,
                    color,
                    plate_pos
                );
                debug!(
                    "CooperativeDoorPassageStrategy: P{} position: {:?}, on plate: {}",
                    player_index + 1,
                    waiting_player.position,
                    waiting_player.position == *plate_pos
                );

                // Check if other player has PassThroughDoor goal
                if let Some(Goal::PassThroughDoor(c, door_pos, target_pos)) =
                    other_player.previous_goal.as_ref()
                    && c == color
                {
                    debug!(
                        "CooperativeDoorPassageStrategy: P{} has PassThroughDoor({:?}, neighbor: {:?}, target: {:?})",
                        other_player_index + 1,
                        c,
                        door_pos,
                        target_pos
                    );
                    debug!(
                        "CooperativeDoorPassageStrategy: P{} position: {:?}, at target: {}",
                        other_player_index + 1,
                        other_player.position,
                        other_player.position == *target_pos
                    );

                    // State 1: If passing player hasn't reached target yet, maintain both goals
                    if other_player.position != *target_pos {
                        debug!(
                            "CooperativeDoorPassageStrategy: STATE 1 - P{} navigating to target {:?} (current: {:?}), P{} waiting on plate at {:?}",
                            other_player_index + 1,
                            target_pos,
                            other_player.position,
                            player_index + 1,
                            plate_pos
                        );
                        let mut goals = vec![None; 2];
                        goals[player_index] = Some(Goal::WaitOnPressurePlate(*color, *plate_pos));
                        goals[other_player_index] =
                            Some(Goal::PassThroughDoor(*c, *door_pos, *target_pos));
                        return goals;
                    }

                    // State 2: Passing player reached target, waiting player still on plate
                    // Release waiting player so they can move off the plate
                    if waiting_player.position == *plate_pos {
                        debug!(
                            "CooperativeDoorPassageStrategy: STATE 2 - P{} at target {:?}, P{} on plate at {:?} - RELEASE P{} to leave",
                            other_player_index + 1,
                            target_pos,
                            player_index + 1,
                            plate_pos,
                            player_index + 1
                        );
                        let mut goals = vec![None; 2];
                        goals[player_index] = None; // Release waiting player to move off plate
                        goals[other_player_index] =
                            Some(Goal::PassThroughDoor(*c, *door_pos, *target_pos)); // Passing player stays at target
                        return goals;
                    }

                    // State 3 & 4: Passing player at target, waiting player left plate
                    // Keep passing player at target until door closes
                    debug!(
                        "CooperativeDoorPassageStrategy: STATE 3/4 - P{} at target {:?}, P{} left plate at {:?} - P{} stays until safe",
                        other_player_index + 1,
                        target_pos,
                        player_index + 1,
                        plate_pos,
                        other_player_index + 1
                    );
                    let mut goals = vec![None; 2];
                    goals[player_index] = None; // Waiting player is free
                    goals[other_player_index] =
                        Some(Goal::PassThroughDoor(*c, *door_pos, *target_pos)); // Passing player stays (waits for door to close)
                    return goals;
                } else {
                    // Other player doesn't have PassThroughDoor goal - cooperation may have ended prematurely
                    debug!(
                        "CooperativeDoorPassageStrategy: ⚠️ WARNING - P{} has WaitOnPressurePlate but P{} has no PassThroughDoor goal (has {:?})",
                        player_index + 1,
                        other_player_index + 1,
                        other_player.previous_goal
                    );
                    debug!(
                        "CooperativeDoorPassageStrategy: P{} is {} on plate at {:?}",
                        player_index + 1,
                        if waiting_player.position == *plate_pos {
                            "STILL"
                        } else {
                            "NOT"
                        },
                        plate_pos
                    );
                }
            }

            // Case 2: This player has PassThroughDoor, check if we need to wait for plate to clear
            // This handles the case where waiting player's goal was released but they haven't moved yet
            if let Some(Goal::PassThroughDoor(color, door_pos, target_pos)) =
                world.players[player_index].previous_goal.as_ref()
            {
                let other_player_index = 1 - player_index;
                let passing_player = &world.players[player_index];
                let other_player = &world.players[other_player_index];

                debug!(
                    "CooperativeDoorPassageStrategy: P{} has PassThroughDoor({:?}, neighbor: {:?}, target: {:?})",
                    player_index + 1,
                    color,
                    door_pos,
                    target_pos
                );

                // If passing player is at target, check if other player is still on a plate of matching color
                if passing_player.position == *target_pos {
                    debug!(
                        "CooperativeDoorPassageStrategy: P{} at target {:?}, checking if P{} is on matching plate",
                        player_index + 1,
                        target_pos,
                        other_player_index + 1
                    );

                    // Check all plates of this color to see if other player is on one
                    if let Some(plates) = world.pressure_plates.get_positions(*color) {
                        for &plate_pos in plates {
                            if other_player.position == plate_pos {
                                debug!(
                                    "CooperativeDoorPassageStrategy: P{} at target, P{} on plate at {:?} - P{} stays frozen",
                                    player_index + 1,
                                    other_player_index + 1,
                                    plate_pos,
                                    player_index + 1
                                );
                                let mut goals = vec![None; 2];
                                goals[player_index] =
                                    Some(Goal::PassThroughDoor(*color, *door_pos, *target_pos)); // Passing player stays
                                goals[other_player_index] = None; // Other player is free to move off plate
                                return goals;
                            }
                        }
                    }

                    debug!(
                        "CooperativeDoorPassageStrategy: P{} at target, P{} not on matching plate - cooperation complete",
                        player_index + 1,
                        other_player_index + 1
                    );
                }
            }
        }

        debug!("CooperativeDoorPassageStrategy: No active cooperation found");

        vec![None; world.players.len()]
    }
}

impl SelectGoal for PickupHealthStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 10 || world.health.is_empty() {
            return vec![None; world.players.len()];
        }

        // Find the best player to pick up health:
        // 1. Player with lowest health
        // 2. If equal health, player closest to health
        // 3. Must be reachable by the player

        let mut best_player: Option<(usize, i32, usize)> = None; // (player_index, health, distance)
        let mut health_pos: Option<Position> = None;

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if current_goals[player_index].is_some() {
                continue;
            }

            // Check if any enemy is close (within 3 tiles actual path distance)
            let enemy_nearby =
                world.enemies.get_positions().iter().any(|&enemy_pos| {
                    world.path_distance_to_enemy(player.position, enemy_pos) <= 2
                });

            if enemy_nearby {
                continue;
            }

            // Find closest reachable health for this player
            if let Some(closest_health) = world.closest_health(player)
                && let Some(path) = world.map.find_path(player.position, closest_health)
            {
                let distance = path.len();
                let should_select = match best_player {
                    None => true,
                    Some((_, best_health, best_distance)) => {
                        // Prefer player with lower health
                        if player.health < best_health {
                            true
                        } else if player.health == best_health {
                            // If equal health, prefer closer player
                            distance < best_distance
                        } else {
                            false
                        }
                    }
                };

                if should_select {
                    best_player = Some((player_index, player.health, distance));
                    health_pos = Some(closest_health);
                }
            }
        }

        let mut goals = vec![None; world.players.len()];
        if let Some((player_index, _, _)) = best_player {
            debug!(
                "Player {} selected for PickupHealth (health={}, pos={:?})",
                player_index + 1,
                world.players[player_index].health,
                health_pos
            );
            goals[player_index] = Some(Goal::PickupHealth);
        }
        goals
    }
}

impl SelectGoal for PickupSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 10 || world.swords.is_empty() {
            return vec![None; world.players.len()];
        }

        // Find the best player to pick up sword:
        // 1. Player must not have a sword already
        // 2. Among players without sword, prefer the closest to any sword
        // 3. Must be reachable by the player

        let mut best_player: Option<(usize, usize, Position)> = None; // (player_index, distance, sword_pos)

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if current_goals[player_index].is_some() {
                continue;
            }

            // Skip if player already has a sword
            if player.has_sword {
                continue;
            }

            // Find the closest reachable sword for this player
            if let Some(closest_sword_pos) = world.closest_sword(player)
                && let Some(path) = world.map.find_path(player.position, closest_sword_pos)
            {
                let distance = path.len();
                let should_select = match best_player {
                    None => true,
                    Some((_, best_distance, _)) => {
                        // Prefer closer player
                        distance < best_distance
                    }
                };

                if should_select {
                    best_player = Some((player_index, distance, closest_sword_pos));
                }
            }
        }

        let mut goals = vec![None; world.players.len()];
        if let Some((player_index, _, sword_pos)) = best_player {
            debug!(
                "Player {} selected for PickupSword (has_sword={}, closest_sword={:?})",
                player_index + 1,
                world.players[player_index].has_sword,
                sword_pos
            );
            goals[player_index] = Some(Goal::PickupSword);
        }
        goals
    }
}

impl SelectGoal for DropBoulderOnPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 6 {
            return vec![None; world.players.len()];
        }

        let mut goals = Vec::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                goals.push(None);
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::Boulder {
                goals.push(None);
                continue;
            }

            debug!("Player carrying a boulder, checking for pressure plates");

            // Find the closest reachable pressure plate across all colors
            let mut closest_plate: Option<(Color, Position, i32)> = None; // (color, position, distance)

            for color in [Color::Red, Color::Green, Color::Blue] {
                if let Some(plates) = world.pressure_plates.get_positions(color) {
                    for &plate_pos in plates {
                        // Check if we can reach the plate
                        if let Some(path_len) = world.path_distance(player.position, plate_pos)
                            && (closest_plate.is_none() || path_len < closest_plate.unwrap().2)
                        {
                            closest_plate = Some((color, plate_pos, path_len));
                        }
                    }
                }
            }

            if let Some((color, plate_pos, _)) = closest_plate {
                debug!("Found closest reachable {:?} pressure plate at {:?}", color, plate_pos);
                goals.push(Some(Goal::DropBoulderOnPlate(color, plate_pos)));
            } else {
                goals.push(None);
            }
        }

        goals
    }
}

impl SelectGoal for DropBoulderStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 6 {
            return vec![None; world.players.len()];
        }

        let mut goals = Vec::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                goals.push(None);
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::Boulder {
                goals.push(None);
                continue;
            }

            debug!("No pressure plates in level, need to drop boulder");
            goals.push(Some(Goal::DropBoulder));
        }

        goals
    }
}

impl SelectGoal for CooperativeDoorPassageStrategySetup {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("CoopPressurePlateDoorStrategySetup: Starting evaluation");

        // Only works in 2-player mode
        if world.players.len() != 2 {
            debug!(
                "CoopPressurePlateDoorStrategySetup: Not 2-player mode ({}), skipping",
                world.players.len()
            );
            return vec![None; world.players.len()];
        }

        // STABILITY: If cooperative goals are already in progress, maintain them
        // Check if we have an active cooperation (one waiting, one passing through)
        let has_active_cooperation = world.players.iter().enumerate().any(|(idx, p)| {
            if let Some(Goal::WaitOnPressurePlate(color, plate_pos)) = &p.previous_goal {
                let other_idx = 1 - idx;
                if let Some(Goal::PassThroughDoor(other_color, door_pos, target_pos)) =
                    &world.players[other_idx].previous_goal
                    && color == other_color {
                        debug!(
                            "CoopPressurePlateDoorStrategySetup: Active cooperation detected - P{} waiting on plate at {:?}, P{} passing through door at {:?} to {:?}",
                            idx + 1,
                            plate_pos,
                            other_idx + 1,
                            door_pos,
                            target_pos
                        );
                        return true;
                    }
            }
            false
        });

        if has_active_cooperation {
            debug!(
                "CoopPressurePlateDoorStrategySetup: Maintaining active cooperation, not reassigning"
            );
            // Let CooperativeDoorPassageStrategy handle maintaining the goals
            return vec![None; 2];
        }

        // Don't assign NEW coop goals if either player already has a goal
        if current_goals.iter().any(|g| g.is_some()) {
            debug!(
                "CoopPressurePlateDoorStrategySetup: Some players already have goals, skipping new assignment"
            );
            return vec![None; world.players.len()];
        }

        // Only use this strategy when both players have no unexplored frontier
        // This ensures we only use cooperative door passage when exploration is complete
        let any_player_has_frontier = world
            .players
            .iter()
            .any(|p| !p.unexplored_frontier.is_empty());
        if any_player_has_frontier {
            debug!(
                "CoopPressurePlateDoorStrategySetup: Some players still have unexplored frontier, continuing exploration first"
            );
            return vec![None; world.players.len()];
        }

        // Check if there are any boulders known
        let has_boulders = world
            .map
            .iter()
            .any(|(_, tile)| matches!(tile, crate::swoq_interface::Tile::Boulder));

        debug!("CoopPressurePlateDoorStrategySetup: Has boulders: {}", has_boulders);

        // Only use this strategy if no boulders are known
        if has_boulders {
            debug!(
                "CoopPressurePlateDoorStrategySetup: Boulders known, preferring boulder solution"
            );
            return vec![None; world.players.len()];
        }

        // Find a pressure plate and door of the same color
        for color in [Color::Red, Color::Green, Color::Blue] {
            debug!("CoopPressurePlateDoorStrategySetup: Checking {:?} color", color);

            // Skip if any player has a key for this color
            if world.players.iter().any(|p| world.has_key(p, color)) {
                debug!("CoopPressurePlateDoorStrategySetup: Player has {:?} key, skipping", color);
                continue;
            }

            let door_positions = match world.doors.get_positions(color) {
                Some(pos) => pos,
                None => {
                    debug!("CoopPressurePlateDoorStrategySetup: No {:?} doors found", color);
                    continue;
                }
            };
            if door_positions.is_empty() {
                debug!("CoopPressurePlateDoorStrategySetup: {:?} door positions empty", color);
                continue;
            }

            debug!(
                "CoopPressurePlateDoorStrategySetup: Found {} {:?} doors",
                door_positions.len(),
                color
            );

            let plates = match world.pressure_plates.get_positions(color) {
                Some(p) => p,
                None => {
                    debug!(
                        "CoopPressurePlateDoorStrategySetup: No {:?} pressure plates found",
                        color
                    );
                    continue;
                }
            };

            debug!("CoopPressurePlateDoorStrategySetup: Found {} {:?} plates", plates.len(), color);

            // Find a pressure plate that's reachable by at least one player
            for &plate_pos in plates {
                debug!("CoopPressurePlateDoorStrategySetup: Checking plate at {:?}", plate_pos);

                // Check if there's a door that's NOT adjacent to this plate
                // (we want to find doors that need someone to wait on the plate)
                for &door_pos in door_positions {
                    // Check if both can reach their targets and calculate path distances
                    let p0_path_to_plate =
                        world.map.find_path(world.players[0].position, plate_pos);
                    let p1_path_to_plate =
                        world.map.find_path(world.players[1].position, plate_pos);

                    let p0_can_reach_plate = p0_path_to_plate.is_some();
                    let p1_can_reach_plate = p1_path_to_plate.is_some();

                    // Calculate actual path distances (use large value if unreachable)
                    let p0_to_plate = p0_path_to_plate
                        .as_ref()
                        .map(|p| p.len())
                        .unwrap_or(i32::MAX as usize);
                    let p1_to_plate = p1_path_to_plate
                        .as_ref()
                        .map(|p| p.len())
                        .unwrap_or(i32::MAX as usize);

                    debug!(
                        "CoopPressurePlateDoorStrategySetup: P1 path dist to plate: {}, P2 path dist to plate: {}",
                        if p0_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p0_to_plate.to_string()
                        },
                        if p1_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p1_to_plate.to_string()
                        }
                    );

                    // For door, find paths to adjacent empty tiles and store them for reuse
                    let p0_path_to_door = door_pos.neighbors().iter().find_map(|&neighbor| {
                        if matches!(
                            world.map.get(&neighbor),
                            Some(crate::swoq_interface::Tile::Empty)
                        ) {
                            world.map.find_path(world.players[0].position, neighbor)
                        } else {
                            None
                        }
                    });
                    let p1_path_to_door = door_pos.neighbors().iter().find_map(|&neighbor| {
                        if matches!(
                            world.map.get(&neighbor),
                            Some(crate::swoq_interface::Tile::Empty)
                        ) {
                            world.map.find_path(world.players[1].position, neighbor)
                        } else {
                            None
                        }
                    });

                    let p0_can_reach_door = p0_path_to_door.is_some();
                    let p1_can_reach_door = p1_path_to_door.is_some();

                    debug!(
                        "CoopPressurePlateDoorStrategySetup: P1 can reach plate: {}, door: {}",
                        p0_can_reach_plate, p0_can_reach_door
                    );
                    debug!(
                        "CoopPressurePlateDoorStrategySetup: P2 can reach plate: {}, door: {}",
                        p1_can_reach_plate, p1_can_reach_door
                    );

                    // CRITICAL: Both players must be able to reach BOTH the plate AND the door
                    // This ensures they're on the same side of the door (not separated by it)
                    let both_can_reach_plate = p0_can_reach_plate && p1_can_reach_plate;
                    let both_can_reach_door = p0_can_reach_door && p1_can_reach_door;

                    if !both_can_reach_plate || !both_can_reach_door {
                        debug!(
                            "CoopPressurePlateDoorStrategySetup: Players on different sides of door - P1 plate:{} door:{}, P2 plate:{} door:{}",
                            p0_can_reach_plate,
                            p0_can_reach_door,
                            p1_can_reach_plate,
                            p1_can_reach_door
                        );
                        continue;
                    }

                    // Assign roles: closer player to plate waits, other goes through door
                    if p0_to_plate <= p1_to_plate {
                        // P1 waits, P2 passes through
                        // Calculate target position from P2's approach path (already calculated)
                        if let Some(path_to_door) = p1_path_to_door
                            && !path_to_door.is_empty()
                        {
                            // Last position in path is adjacent to door (the neighbor)
                            let last = path_to_door[path_to_door.len() - 1];
                            // Get direction from neighbor to door
                            let dx = door_pos.x - last.x;
                            let dy = door_pos.y - last.y;
                            debug!(
                                "CoopPressurePlateDoorStrategySetup: Door at {:?}, last neighbor at {:?}, direction dx={}, dy={}",
                                door_pos, last, dx, dy
                            );

                            // Target is one step beyond the door in the same direction
                            let target_pos = Position {
                                x: door_pos.x + dx,
                                y: door_pos.y + dy,
                            };

                            // CRITICAL CHECK: Only use this strategy if target_pos cannot be reached by any other route
                            // Check if either player can already reach the target without going through the door
                            let p0_can_reach_target = world
                                .map
                                .find_path(world.players[0].position, target_pos)
                                .is_some();
                            let p1_can_reach_target = world
                                .map
                                .find_path(world.players[1].position, target_pos)
                                .is_some();

                            if p0_can_reach_target || p1_can_reach_target {
                                debug!(
                                    "CoopPressurePlateDoorStrategySetup: Target {:?} is already reachable (P1: {}, P2: {}), no cooperation needed",
                                    target_pos, p0_can_reach_target, p1_can_reach_target
                                );
                                continue;
                            }

                            debug!(
                                "CoopPressurePlateDoorStrategySetup: ✓ SELECTED - P1 waits on {:?} plate at {:?}, P2 goes through door at {:?} to target {:?}",
                                color, plate_pos, door_pos, target_pos
                            );
                            return vec![
                                Some(Goal::WaitOnPressurePlate(color, plate_pos)),
                                Some(Goal::PassThroughDoor(color, last, target_pos)),
                            ];
                        }

                        debug!(
                            "CoopPressurePlateDoorStrategySetup: Could not calculate valid target for P2 through door at {:?}",
                            door_pos
                        );
                        continue;
                    } else {
                        // P2 waits, P1 passes through
                        // Calculate target position from P1's approach path (already calculated)
                        if let Some(path_to_door) = p0_path_to_door
                            && !path_to_door.is_empty()
                        {
                            // Last position in path is adjacent to door (the neighbor)
                            let last = path_to_door[path_to_door.len() - 1];
                            // Get direction from neighbor to door
                            let dx = door_pos.x - last.x;
                            let dy = door_pos.y - last.y;
                            debug!(
                                "CoopPressurePlateDoorStrategySetup: Door at {:?}, last neighbor at {:?}, direction dx={}, dy={}",
                                door_pos, last, dx, dy
                            );

                            // Target is one step beyond the door in the same direction
                            let target_pos = Position {
                                x: door_pos.x + dx,
                                y: door_pos.y + dy,
                            };

                            // CRITICAL CHECK: Only use this strategy if target_pos cannot be reached by any other route
                            // Check if either player can already reach the target without going through the door
                            let p0_can_reach_target = world
                                .map
                                .find_path(world.players[0].position, target_pos)
                                .is_some();
                            let p1_can_reach_target = world
                                .map
                                .find_path(world.players[1].position, target_pos)
                                .is_some();

                            if p0_can_reach_target || p1_can_reach_target {
                                debug!(
                                    "CoopPressurePlateDoorStrategySetup: Target {:?} is already reachable (P1: {}, P2: {}), no cooperation needed",
                                    target_pos, p0_can_reach_target, p1_can_reach_target
                                );
                                continue;
                            }

                            debug!(
                                "CoopPressurePlateDoorStrategySetup: ✓ SELECTED - P2 waits on {:?} plate at {:?}, P1 goes through door at {:?} to target {:?}",
                                color, plate_pos, door_pos, target_pos
                            );
                            return vec![
                                Some(Goal::PassThroughDoor(color, last, target_pos)),
                                Some(Goal::WaitOnPressurePlate(color, plate_pos)),
                            ];
                        }

                        debug!(
                            "CoopPressurePlateDoorStrategySetup: Could not calculate valid target for P1 through door at {:?}",
                            door_pos
                        );
                        continue;
                    }
                }
            }
        }

        debug!("CoopPressurePlateDoorStrategySetup: No suitable plate/door combination found");
        vec![None; world.players.len()]
    }
}

impl SelectGoal for UsePressurePlateForDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = Vec::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                goals.push(None);
                continue;
            }
            let mut player_goal = None;

            // Check if we can use pressure plates to open doors (prefer keys over plates)
            for color in [Color::Red, Color::Green, Color::Blue] {
                // Skip if we have a key for this color
                if world.has_key(player, color) {
                    continue;
                }

                let door_positions = match world.doors.get_positions(color) {
                    Some(pos) => pos,
                    None => continue,
                };
                if door_positions.is_empty() {
                    continue;
                }

                // Get pressure plates for this color
                if let Some(plates) = world.pressure_plates.get_positions(color) {
                    // Check each pressure plate to see if we can stand on it
                    for &plate_pos in plates {
                        // Can we reach the plate?
                        if !player.position.is_adjacent(&plate_pos)
                            && world.map.find_path(player.position, plate_pos).is_none()
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
                                player_goal = Some(Goal::WaitOnPressurePlate(color, plate_pos));
                                break;
                            }
                        }
                        if player_goal.is_some() {
                            break;
                        }
                    }
                }
                if player_goal.is_some() {
                    break;
                }
            }

            goals.push(player_goal);
        }

        goals
    }
}

impl SelectGoal for OpenDoorWithKeyStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Track which door colors have been assigned to prevent conflicts
        let mut assigned_door_colors = std::collections::HashSet::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }

            for &color in world.doors.colors() {
                // Skip if this door color is already assigned to another player
                if assigned_door_colors.contains(&color) {
                    continue;
                }

                if world.has_key(player, color) {
                    let door_positions = match world.doors.get_positions(color) {
                        Some(pos) => pos,
                        None => continue,
                    };

                    // Check if any door of this color has a reachable empty neighbor
                    for &door_pos in door_positions {
                        debug!(
                            "Player {} checking door {:?} at {:?}",
                            player_index + 1,
                            color,
                            door_pos
                        );

                        // Check if any neighbor of the door is reachable
                        let has_reachable_neighbor = door_pos.neighbors().iter().any(|&neighbor| {
                            // Only consider empty tiles (or player position)
                            if neighbor != player.position
                                && !matches!(
                                    world.map.get(&neighbor),
                                    Some(crate::swoq_interface::Tile::Empty)
                                )
                            {
                                return false;
                            }

                            // Check if player is already at this neighbor or can path to it
                            player.position == neighbor
                                || world.map.find_path(player.position, neighbor).is_some()
                        });

                        if has_reachable_neighbor {
                            debug!(
                                "Player {} assigned to open {:?} door (has key, door reachable)",
                                player_index + 1,
                                color
                            );
                            goals[player_index] = Some(Goal::OpenDoor(color));
                            assigned_door_colors.insert(color);
                            break;
                        } else {
                            debug!("Door at {:?} has no reachable empty neighbors", door_pos);
                        }
                    }
                }
            }
        }

        goals
    }
}

impl SelectGoal for GetKeyForDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Track which key colors have been assigned to prevent conflicts
        let mut assigned_key_colors = std::collections::HashSet::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }

            for color in world.doors_without_keys(player) {
                // Skip if this key color is already assigned to another player
                if assigned_key_colors.contains(&color) {
                    continue;
                }

                debug!("Player {} checking door without key: {:?}", player_index + 1, color);

                // If we know where the key is and can reach it, go get it
                debug!(
                    "Checking if we know key location for {:?}: {}",
                    color,
                    world.knows_key_location(color)
                );
                if world.knows_key_location(color) {
                    if let Some(key_pos) = world.closest_key(player, color) {
                        debug!("Closest key for {:?} is at {:?}", color, key_pos);
                        // Use can_open_doors=true to allow using keys we already have
                        // Use avoid_keys=true to not pick up other keys along the way
                        if world.map.find_path(player.position, key_pos).is_some() {
                            debug!(
                                "Player {} assigned to get {:?} key (reachable)",
                                player_index + 1,
                                color
                            );
                            goals[player_index] = Some(Goal::GetKey(color));
                            assigned_key_colors.insert(color);
                            break;
                        } else {
                            debug!("Key at {:?} is not reachable", key_pos);
                        }
                    } else {
                        debug!("No keys found for {:?}!", color);
                    }
                }
            }
        }

        goals
    }
}

impl SelectGoal for ReachExitStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::SinglePlayer
    }

    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        let exit_pos = world.exit_position?;

        // For 2-player mode, check if all active players can reach the exit before anyone tries
        if world.players.len() > 1 {
            let active_players: Vec<&crate::player_state::PlayerState> =
                world.players.iter().filter(|p| p.is_active).collect();

            // If we have multiple active players, check if all can reach the exit
            if active_players.len() > 1 {
                let all_can_reach = active_players.iter().all(|p| {
                    p.inventory == crate::swoq_interface::Inventory::Boulder
                        || world.map.find_path(p.position, exit_pos).is_some()
                });

                // If not all active players can reach the exit, don't assign exit goal to anyone
                if !all_can_reach {
                    debug!(
                        "2-player mode: Not all active players can reach exit, continuing exploration"
                    );
                    return None;
                }
            }
        }

        // Check if we're carrying a boulder - must drop it before exiting
        if player.inventory == crate::swoq_interface::Inventory::Boulder {
            debug!("Need to drop boulder before reaching exit");
            return Some(Goal::DropBoulder);
        }

        // Check if we can actually path to the exit
        if world.map.find_path(player.position, exit_pos).is_some() {
            Some(Goal::ReachExit)
        } else {
            debug!("Exit at {:?} is not reachable, continuing exploration", exit_pos);
            None
        }
    }
}

impl SelectGoal for FetchBoulderForPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        if world.level < 6 || world.boulders.is_empty() {
            return goals;
        }

        // First priority: if there's a pressure plate, fetch a boulder for it
        let has_pressure_plates = [Color::Red, Color::Green, Color::Blue]
            .iter()
            .any(|&color| world.pressure_plates.has_color(color));

        if !has_pressure_plates {
            return goals;
        }

        // Track which boulders have been assigned to prevent conflicts
        let mut assigned_boulders = std::collections::HashSet::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::None {
                continue;
            }

            debug!(
                "Player {} found pressure plates, looking for nearest boulder",
                player_index + 1
            );

            // Find nearest reachable boulder that hasn't been assigned
            let mut nearest_boulder: Option<Position> = None;
            let mut nearest_distance = i32::MAX;

            for boulder_pos in world.boulders.get_all_positions() {
                // Skip if this boulder is already assigned to another player
                if assigned_boulders.contains(&boulder_pos) {
                    continue;
                }

                let dist = player.position.distance(&boulder_pos);
                if dist < nearest_distance {
                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        world.map.is_walkable(&adj, adj)
                            && world.map.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        nearest_boulder = Some(boulder_pos);
                        nearest_distance = dist;
                    }
                }
            }

            if let Some(boulder_pos) = nearest_boulder {
                debug!(
                    "Player {} assigned boulder at {:?} for pressure plate",
                    player_index + 1,
                    boulder_pos
                );
                goals[player_index] = Some(Goal::FetchBoulder(boulder_pos));
                assigned_boulders.insert(boulder_pos);
            }
        }

        goals
    }
}

impl SelectGoal for MoveUnexploredBoulderStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        if world.level < 6 || world.boulders.is_empty() {
            return goals;
        }

        // Track which boulders have been assigned to prevent conflicts
        let mut assigned_boulders = std::collections::HashSet::new();

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }
            if player.inventory != crate::swoq_interface::Inventory::None {
                continue;
            }

            debug!(
                "Player {} checking {} boulders for unexplored ones (frontier size: {})",
                player_index + 1,
                world.boulders.len(),
                player.unexplored_frontier.len()
            );

            // Check if any boulder is unexplored and reachable
            for boulder_pos in world.boulders.get_original_boulders() {
                // Skip if this boulder is already assigned to another player
                if assigned_boulders.contains(&boulder_pos) {
                    continue;
                }

                // Is the boulder unexplored (not moved by us)?
                if !world.boulders.has_moved(&boulder_pos) {
                    debug!("  Boulder at {:?} is unexplored", boulder_pos);

                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        world.map.is_walkable(&adj, adj)
                            && world.map.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        debug!(
                            "  Player {} assigned boulder at {:?}",
                            player_index + 1,
                            boulder_pos
                        );
                        goals[player_index] = Some(Goal::FetchBoulder(boulder_pos));
                        assigned_boulders.insert(boulder_pos);
                        break;
                    } else {
                        debug!("  Boulder at {:?} is not reachable yet", boulder_pos);
                    }
                }
            }

            if goals[player_index].is_none() {
                debug!("Player {} found no reachable unexplored boulders", player_index + 1);
            }
        }

        goals
    }
}

impl SelectGoal for FallbackPressurePlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::SinglePlayer
    }

    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        // If nothing else to do and area is fully explored, step on any reachable pressure plate
        if !player.unexplored_frontier.is_empty() {
            return None;
        }

        debug!("Area fully explored, checking for pressure plates to step on");
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plates) = world.pressure_plates.get_positions(color) {
                for &plate_pos in plates {
                    // Check if we can reach the plate
                    if player.position.is_adjacent(&plate_pos)
                        || world.map.find_path(player.position, plate_pos).is_some()
                    {
                        debug!(
                            "Found reachable {:?} pressure plate at {:?} as fallback",
                            color, plate_pos
                        );
                        return Some(Goal::WaitOnPressurePlate(color, plate_pos));
                    }
                }
            }
        }

        None
    }
}

impl SelectGoal for HuntEnemyWithSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::SinglePlayer
    }

    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        // Only hunt enemies when:
        // 1. We have a sword
        // 2. The entire maze is explored (frontier is empty)
        // 3. There are enemies or potential enemy locations
        debug!(
            "HuntEnemyWithSwordStrategy check: has_sword={}, frontier_empty={}, enemies_present={} (count={}), potential_enemies={} (count={})",
            player.has_sword,
            player.unexplored_frontier.is_empty(),
            !world.enemies.is_empty(),
            world.enemies.get_positions().len(),
            !world.potential_enemy_locations.is_empty(),
            world.potential_enemy_locations.len()
        );

        if !player.has_sword
            || !player.unexplored_frontier.is_empty()
            || (world.enemies.is_empty() && world.potential_enemy_locations.is_empty())
        {
            return None;
        }

        debug!("Maze fully explored, have sword, hunting enemy (may drop key)");

        // Find the closest enemy
        if let Some(enemy_pos) = world.closest_enemy(player) {
            debug!("Hunting known enemy at {:?}", enemy_pos);
            return Some(Goal::KillEnemy(enemy_pos));
        }

        // If no known enemies, hunt potential enemy locations
        if let Some(potential_pos) = world.closest_potential_enemy(player) {
            debug!("No known enemies, hunting potential enemy location at {:?}", potential_pos);
            return Some(Goal::KillEnemy(potential_pos));
        }

        None
    }
}

impl SelectGoal for RandomExploreStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::SinglePlayer
    }

    fn try_select(&self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let player = &world.players[player_index];

        // Only use random exploration when:
        // 1. The frontier is empty (nothing new to explore)
        // 2. We're not doing anything else
        if !player.unexplored_frontier.is_empty() {
            return None;
        }

        // If we already have a RandomExplore goal and destination, keep it
        if let Some(Goal::RandomExplore(_)) = &player.previous_goal
            && player.current_destination.is_some()
        {
            debug!("RandomExploreStrategy: Continuing with existing destination");
            return player.previous_goal.clone();
        }

        debug!("RandomExploreStrategy: Frontier empty, selecting random reachable position");

        // Collect all empty positions that we've seen
        let empty_positions: Vec<Position> = world
            .map
            .iter()
            .filter_map(|(pos, tile)| {
                if matches!(tile, crate::swoq_interface::Tile::Empty)
                    && player.position.distance(pos) > 5
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
            if world.map.find_path(player.position, target).is_some() {
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
