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
                world.players[player_index].current_goal = None;
                world.players[player_index].previous_goal = None;
            }
        }

        println!("\n┌────────────────────────────────────────────────────────────┐");
        println!("│ 🧠 PLANNING PHASE - Selecting goals                        ");
        println!("└────────────────────────────────────────────────────────────┘");

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
        let mut actions: Vec<DirectedAction> = Vec::new();
        for (player_index, goal) in goals.iter().enumerate() {
            if player_index < num_players {
                // Set current goal before execution
                world.players[player_index].current_goal = Some(goal.clone());
                let action = goal
                    .execute_for_player(world, player_index)
                    .unwrap_or(DirectedAction::None);
                actions.push(action);
            }
        }

        // Check for conflicts and resolve them
        // If both players would conflict, only delay the higher-indexed player to ensure progress
        let mut delayed_player: Option<usize> = None;

        if num_players > 1 {
            for player_index in 0..num_players {
                let action = actions[player_index];
                if !matches!(
                    action,
                    DirectedAction::MoveNorth
                        | DirectedAction::MoveSouth
                        | DirectedAction::MoveEast
                        | DirectedAction::MoveWest
                ) {
                    continue;
                }

                let player_pos = world.players[player_index].position;
                let expected_next = match action {
                    DirectedAction::MoveNorth => Position::new(player_pos.x, player_pos.y - 1),
                    DirectedAction::MoveSouth => Position::new(player_pos.x, player_pos.y + 1),
                    DirectedAction::MoveEast => Position::new(player_pos.x + 1, player_pos.y),
                    DirectedAction::MoveWest => Position::new(player_pos.x - 1, player_pos.y),
                    _ => player_pos,
                };

                for other_index in 0..num_players {
                    if other_index == player_index {
                        continue;
                    }

                    let other_pos = world.players[other_index].position;

                    // Check if we're trying to move to where another player is
                    if expected_next == other_pos {
                        debug!(
                            "Player {} conflicts: trying to move to player {}'s current position {:?}",
                            player_index + 1,
                            other_index + 1,
                            other_pos
                        );

                        // If no player has been delayed yet, delay this player
                        if delayed_player.is_none() {
                            delayed_player = Some(player_index);
                            actions[player_index] = DirectedAction::None;
                            debug!("Player {} action delayed", player_index + 1);
                        }
                        break;
                    }

                    // Check if we're trying to move to where another player is going (their next step)
                    if let Some(ref other_path) = world.players[other_index].current_path
                        && other_path.len() >= 2
                    {
                        let other_next = other_path[1];
                        if expected_next == other_next {
                            debug!(
                                "Player {} conflicts: trying to move to player {}'s next position {:?}",
                                player_index + 1,
                                other_index + 1,
                                other_next
                            );

                            // If no player has been delayed yet, delay this player
                            if delayed_player.is_none() {
                                delayed_player = Some(player_index);
                                actions[player_index] = DirectedAction::None;
                                debug!("Player {} action delayed", player_index + 1);
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Consistency check and build results
        let mut results = Vec::new();
        for (player_index, goal) in goals.into_iter().enumerate() {
            if player_index < num_players {
                let action = actions[player_index];
                let player_pos = world.players[player_index].position;
                let expected_next = match action {
                    DirectedAction::MoveNorth => Position::new(player_pos.x, player_pos.y - 1),
                    DirectedAction::MoveSouth => Position::new(player_pos.x, player_pos.y + 1),
                    DirectedAction::MoveEast => Position::new(player_pos.x + 1, player_pos.y),
                    DirectedAction::MoveWest => Position::new(player_pos.x - 1, player_pos.y),
                    _ => player_pos,
                };

                // Consistency check: verify action matches destination
                if let Some(_dest) = world.players[player_index].current_destination {
                    // Check if the action is moving towards the destination
                    if action != DirectedAction::None
                        && matches!(
                            action,
                            DirectedAction::MoveNorth
                                | DirectedAction::MoveSouth
                                | DirectedAction::MoveEast
                                | DirectedAction::MoveWest
                        )
                    {
                        // Verify the move is on a valid path to destination
                        if let Some(ref path) = world.players[player_index].current_path
                            && path.len() >= 2
                            && path[1] != expected_next
                        {
                            debug!(
                                "WARNING: Action {:?} leads to {:?} but path expects {:?}",
                                action, expected_next, path[1]
                            );
                        }
                    }
                }

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
            &AttackOrFleeEnemyStrategy,
            &PickupHealthStrategy,
            &PickupSwordStrategy,
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
pub struct PickupHealthStrategy;
pub struct PickupSwordStrategy;
pub struct DropBoulderOnPlateStrategy;
pub struct DropBoulderStrategy;
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
                                player_goal = Some(Goal::StepOnPressurePlate(color, plate_pos));
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
                        return Some(Goal::StepOnPressurePlate(color, plate_pos));
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
        if let Some(Goal::RandomExplore(_)) = &player.current_goal
            && player.current_destination.is_some()
        {
            debug!("RandomExploreStrategy: Continuing with existing destination");
            return player.current_goal.clone();
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
