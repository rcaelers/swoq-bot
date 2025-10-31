use std::collections::HashMap;
use tracing::debug;

use crate::goal::Goal;
use crate::types::{Color, Position};
use crate::world_state::WorldState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrategyType {
    /// Strategy is executed once per player independently.
    /// Each player that doesn't have a goal is evaluated and may select this strategy or not.
    Individual,
    /// Strategy is evaluated once for all players cooperatively.
    /// Each player is evaluated independently and may get different goals or no goal.
    Coop,
}

/// Container for all strategy instances, created once per level
pub struct StrategyPlanner {
    strategies: Vec<Box<dyn SelectGoal>>,
    /// Track which strategy index selected each player's goal from the previous tick
    last_strategy_per_player: Vec<Option<usize>>,
}

impl StrategyPlanner {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(AttackOrFleeEnemyStrategy),
                Box::new(PickupHealthStrategy),
                Box::new(PickupSwordStrategy),
                Box::new(CooperativeDoorPassageStrategy::new()),
                Box::new(DropBoulderOnPlateStrategy),
                Box::new(DropBoulderStrategy),
                Box::new(UsePressurePlateForDoorStrategy),
                Box::new(OpenDoorWithKeyStrategy),
                Box::new(GetKeyForDoorStrategy),
                Box::new(ReachExitStrategy),
                Box::new(FetchBoulderForPlateStrategy),
                Box::new(MoveUnexploredBoulderStrategy),
                Box::new(FallbackPressurePlateStrategy),
                Box::new(HuntEnemyWithSwordStrategy),
                Box::new(RandomExploreStrategy),
            ],
            last_strategy_per_player: Vec::new(),
        }
    }

    #[tracing::instrument(level = "debug", skip(self, world))]
    pub fn select_goal(&mut self, world: &WorldState) -> Vec<Goal> {
        let num_players = world.players.len();
        let mut selected_goals: Vec<Option<Goal>> = vec![None; num_players];
        let mut current_strategy_per_player: Vec<Option<usize>> = vec![None; num_players];
        
        // Initialize last_strategy_per_player if needed
        if self.last_strategy_per_player.len() != num_players {
            self.last_strategy_per_player = vec![None; num_players];
        }

        // First, try to prioritize strategies that were used last tick
        let unique_last_strategies: std::collections::HashSet<usize> = 
            self.last_strategy_per_player.iter().filter_map(|&s| s).collect();
        
        for &strategy_idx in &unique_last_strategies {
            if strategy_idx >= self.strategies.len() {
                continue;
            }
            
            let strategy = &mut self.strategies[strategy_idx];
            if !strategy.prioritize(world) {
                continue;
            }
            
            debug!("Prioritizing strategy {} from previous tick", strategy_idx);
            
            Self::process_strategy(
                strategy,
                strategy_idx,
                world,
                &mut selected_goals,
                &mut current_strategy_per_player,
                num_players,
                true,
            );
        }

        // Process remaining strategies in order
        for (strategy_idx, strategy) in self.strategies.iter_mut().enumerate() {
            Self::process_strategy(
                strategy,
                strategy_idx,
                world,
                &mut selected_goals,
                &mut current_strategy_per_player,
                num_players,
                false,
            );
            
            // If all players have goals, we're done
            if selected_goals.iter().all(|g| g.is_some()) {
                break;
            }
        }

        // Store which strategies selected goals for next tick
        self.last_strategy_per_player = current_strategy_per_player;

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

    pub fn all_players_have_no_goals(goals: &[Option<Goal>]) -> bool {
        goals.iter().all(|g| g.is_none())
    }

    /// Check if any player still needs a goal
    fn any_player_needs_goal(selected_goals: &[Option<Goal>]) -> bool {
        selected_goals.iter().any(|g| g.is_none())
    }

    /// Process a strategy and assign goals to players
    fn process_strategy(
        strategy: &mut Box<dyn SelectGoal>,
        strategy_idx: usize,
        world: &WorldState,
        selected_goals: &mut [Option<Goal>],
        current_strategy_per_player: &mut [Option<usize>],
        num_players: usize,
        is_prioritized: bool,
    ) {
        match strategy.strategy_type() {
            StrategyType::Individual => {
                for (player_index, goal_slot) in selected_goals.iter_mut().enumerate() {
                    if goal_slot.is_none()
                        && let Some(goal) = strategy.try_select(world, player_index)
                    {
                        if is_prioritized {
                            debug!("Player {} re-selected goal from prioritized strategy: {:?}", player_index + 1, goal);
                        } else {
                            debug!("Player {} selected goal: {:?}", player_index + 1, goal);
                        }
                        *goal_slot = Some(goal);
                        current_strategy_per_player[player_index] = Some(strategy_idx);
                    }
                }
            }
            StrategyType::Coop => {
                if Self::any_player_needs_goal(selected_goals) {
                    let coop_goals = strategy.try_select_coop(world, selected_goals);
                    for player_index in 0..num_players.min(coop_goals.len()) {
                        if selected_goals[player_index].is_none()
                            && let Some(goal) = &coop_goals[player_index]
                        {
                            if is_prioritized {
                                debug!("Player {} selected co-op goal from prioritized strategy: {:?}", player_index + 1, goal);
                            } else {
                                debug!("Player {} selected co-op goal: {:?}", player_index + 1, goal);
                            }
                            selected_goals[player_index] = Some(goal.clone());
                            current_strategy_per_player[player_index] = Some(strategy_idx);
                        }
                    }
                }
            }
        }
    }
}

pub trait SelectGoal {
    /// Returns the strategy type (Individual or Coop)
    fn strategy_type(&self) -> StrategyType;

    /// Called on strategies that selected goals in the previous tick.
    /// Return true if this strategy should be tried again before other strategies.
    /// Default implementation returns false (no prioritization).
    fn prioritize(&self, world: &WorldState) -> bool {
        let _ = world;
        false
    }

    /// Try to select a goal for a specific player (0 or 1)
    /// For Individual strategies only
    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
        let _ = (world, player_index);
        None
    }

    /// Try to select goals for all players at once
    /// For Coop strategies only. Returns a Vec with one Option<Goal> per player.
    /// Can return different goals for different players, or None for some/all.
    /// `current_goals` contains the already-assigned goals (None if no goal yet).
    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let _ = (world, current_goals);
        vec![None; world.players.len()]
    }
}

pub struct AttackOrFleeEnemyStrategy;

#[derive(Debug, Clone, Copy, PartialEq)]
enum CooperativeDoorPassageState {
    Setup,
    Execute,
}

pub struct CooperativeDoorPassageStrategy {
    state: CooperativeDoorPassageState,
    // Track when each color door was last opened using a plate (tick number)
    last_plate_door_usage: HashMap<Color, i32>,
}

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
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
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

impl CooperativeDoorPassageStrategy {
    pub fn new() -> Self {
        Self {
            state: CooperativeDoorPassageState::Setup,
            last_plate_door_usage: HashMap::new(),
        }
    }

    /// Check if there's an active cooperative door passage in progress
    fn has_active_door_cooperation(&self, world: &WorldState) -> bool {
        world.players.iter().enumerate().any(|(idx, p)| {
            if let Some(Goal::WaitOnPressurePlate(color, plate_pos)) = &p.previous_goal {
                let other_idx = 1 - idx;
                if let Some(Goal::PassThroughDoor(other_color, door_pos, target_pos)) =
                    &world.players[other_idx].previous_goal
                    && color == other_color
                {
                    debug!(
                        "Active cooperation detected - P{} waiting on plate at {:?}, P{} passing through door at {:?} to {:?}",
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
        })
    }

    fn execute_phase(&mut self, world: &WorldState) -> Vec<Option<Goal>> {
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
        
        // If no active cooperation found, transition back to Setup state
        self.state = CooperativeDoorPassageState::Setup;

        vec![None; world.players.len()]
    }

    /// Calculate pathfinding for a single player to the plate and door
    fn player_can_reach_plate_and_door(
        &self,
        world: &WorldState,
        player_pos: Position,
        plate_pos: Position,
        door_pos: Position,
    ) -> PlayerReachability {
        let path_to_plate = world.map.find_path(player_pos, plate_pos);
        let can_reach_plate = path_to_plate.is_some();
        let distance_to_plate = path_to_plate
            .as_ref()
            .map(|p| p.len())
            .unwrap_or(i32::MAX as usize);

        let path_to_door = door_pos.neighbors().iter().find_map(|&neighbor| {
            if matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty)) {
                world.map.find_path(player_pos, neighbor)
            } else {
                None
            }
        });
        let can_reach_door = path_to_door.is_some();

        PlayerReachability {
            can_reach_plate,
            can_reach_door,
            distance_to_plate,
            path_to_door,
        }
    }

    /// Calculate the target position one step beyond the door
    fn calculate_target_beyond_door(
        &self,
        door_pos: Position,
        path_to_door: &[Position],
    ) -> Option<Position> {
        if path_to_door.is_empty() {
            return None;
        }
        let last = path_to_door[path_to_door.len() - 1];
        let dx = door_pos.x - last.x;
        let dy = door_pos.y - last.y;
        Some(Position {
            x: door_pos.x + dx,
            y: door_pos.y + dy,
        })
    }

    /// Check if either player can already reach the target position
    fn is_target_already_reachable(&self, world: &WorldState, target_pos: Position) -> bool {
        world
            .map
            .find_path(world.players[0].position, target_pos)
            .is_some()
            || world
                .map
                .find_path(world.players[1].position, target_pos)
                .is_some()
    }

    fn setup_phase(&mut self, world: &WorldState, current_goals: &[Option<Goal>]) -> Vec<Option<Goal>> {
        debug!("CoopPressurePlateDoorStrategy: Starting setup phase evaluation");

        if !world.is_two_player_mode() {
            debug!(
                "CoopPressurePlateDoorStrategy: Not 2-player mode ({}), skipping",
                world.players.len()
            );
            return vec![None; world.players.len()];
        }

        if !StrategyPlanner::all_players_have_no_goals(current_goals) {
            debug!(
                "CoopPressurePlateDoorStrategy: Some players already have goals, skipping new assignment"
            );
            return vec![None; world.players.len()];
        }

        if world.any_player_has_frontier() {
            debug!(
                "CoopPressurePlateDoorStrategy: Some players still have unexplored frontier, continuing exploration first"
            );
            return vec![None; world.players.len()];
        }

        if world.map.has_boulders() {
            debug!(
                "CoopPressurePlateDoorStrategy: Boulders known, preferring boulder solution"
            );
            return vec![None; world.players.len()];
        }

        // Sort colors by least recently used (prefer colors not used yet or used longest ago)
        let mut colors_by_usage: Vec<Color> = vec![Color::Red, Color::Green, Color::Blue];
        colors_by_usage.sort_by_key(|&color| {
            self
                .last_plate_door_usage
                .get(&color)
                .copied()
                .unwrap_or(i32::MIN)
        });
        debug!(
            "CoopPressurePlateDoorStrategy: Checking colors in priority order: {:?}",
            colors_by_usage
        );

        // Find a pressure plate and door of the same color
        for color in colors_by_usage {
            debug!("CoopPressurePlateDoorStrategy: Checking {:?} color", color);

            // Skip if any player has a key for this color
            if world.players.iter().any(|p| world.has_key(p, color)) {
                debug!("CoopPressurePlateDoorStrategy: Player has {:?} key, skipping", color);
                continue;
            }

            let door_positions = match world.doors.get_positions(color) {
                Some(pos) => pos,
                None => {
                    debug!("CoopPressurePlateDoorStrategy: No {:?} doors found", color);
                    continue;
                }
            };
            if door_positions.is_empty() {
                debug!("CoopPressurePlateDoorStrategy: {:?} door positions empty", color);
                continue;
            }

            debug!(
                "CoopPressurePlateDoorStrategy: Found {} {:?} doors",
                door_positions.len(),
                color
            );

            let plates = match world.pressure_plates.get_positions(color) {
                Some(p) => p,
                None => {
                    debug!(
                        "CoopPressurePlateDoorStrategy: No {:?} pressure plates found",
                        color
                    );
                    continue;
                }
            };

            debug!("CoopPressurePlateDoorStrategy: Found {} {:?} plates", plates.len(), color);

            let last_usage_tick = self
                .last_plate_door_usage
                .get(&color)
                .copied()
                .unwrap_or(i32::MIN);
            debug!(
                "CoopPressurePlateDoorStrategy: {:?} door last used at tick {} (current tick: {})",
                color,
                if last_usage_tick == i32::MIN {
                    "never".to_string()
                } else {
                    last_usage_tick.to_string()
                },
                world.tick
            );

            // Find a pressure plate that's reachable by at least one player
            for &plate_pos in plates {
                debug!("CoopPressurePlateDoorStrategy: Checking plate at {:?}", plate_pos);

                // Check if there's a door that's NOT adjacent to this plate
                // (we want to find doors that need someone to wait on the plate)
                for &door_pos in door_positions {
                    let p0_reach = self.player_can_reach_plate_and_door(
                        world,
                        world.players[0].position,
                        plate_pos,
                        door_pos,
                    );
                    let p1_reach = self.player_can_reach_plate_and_door(
                        world,
                        world.players[1].position,
                        plate_pos,
                        door_pos,
                    );

                    debug!(
                        "CoopPressurePlateDoorStrategy: P1 path dist to plate: {}, P2 path dist to plate: {}",
                        if p0_reach.distance_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p0_reach.distance_to_plate.to_string()
                        },
                        if p1_reach.distance_to_plate == i32::MAX as usize {
                            "unreachable".to_string()
                        } else {
                            p1_reach.distance_to_plate.to_string()
                        }
                    );

                    debug!(
                        "CoopPressurePlateDoorStrategy: P1 can reach plate: {}, door: {}",
                        p0_reach.can_reach_plate, p0_reach.can_reach_door
                    );
                    debug!(
                        "CoopPressurePlateDoorStrategy: P2 can reach plate: {}, door: {}",
                        p1_reach.can_reach_plate, p1_reach.can_reach_door
                    );

                    // CRITICAL: Both players must be able to reach BOTH the plate AND the door
                    let both_can_reach_plate = p0_reach.can_reach_plate && p1_reach.can_reach_plate;
                    let both_can_reach_door = p0_reach.can_reach_door && p1_reach.can_reach_door;

                    if !both_can_reach_plate || !both_can_reach_door {
                        debug!(
                            "CoopPressurePlateDoorStrategy: Players on different sides of door - P1 plate:{} door:{}, P2 plate:{} door:{}",
                            p0_reach.can_reach_plate,
                            p0_reach.can_reach_door,
                            p1_reach.can_reach_plate,
                            p1_reach.can_reach_door
                        );
                        continue;
                    }

                    // Assign roles: closer player to plate waits, other goes through door
                    let (waiter_idx, passer_idx, passer_reach) =
                        if p0_reach.distance_to_plate <= p1_reach.distance_to_plate {
                            (0, 1, &p1_reach)
                        } else {
                            (1, 0, &p0_reach)
                        };

                    if let Some(ref path_to_door) = passer_reach.path_to_door
                        && let Some(target_pos) =
                            self.calculate_target_beyond_door(door_pos, path_to_door)
                    {
                        let last = path_to_door[path_to_door.len() - 1];
                        debug!(
                            "CoopPressurePlateDoorStrategy: Door at {:?}, neighbor at {:?}, target {:?}",
                            door_pos, last, target_pos
                        );

                        if self.is_target_already_reachable(world, target_pos) {
                            debug!(
                                "CoopPressurePlateDoorStrategy: Target {:?} is already reachable, no cooperation needed",
                                target_pos
                            );
                            continue;
                        }

                        // Record this door color as being used with a plate at this tick
                        self
                            .last_plate_door_usage
                            .insert(color, world.tick);
                        
                        // Transition to Execute state
                        self.state = CooperativeDoorPassageState::Execute;

                        debug!(
                            "CoopPressurePlateDoorStrategy: ✓ SELECTED - P{} waits on {:?} plate at {:?}, P{} goes through door at {:?} to target {:?} (last used: tick {})",
                            waiter_idx + 1,
                            color,
                            plate_pos,
                            passer_idx + 1,
                            door_pos,
                            target_pos,
                            world.tick
                        );

                        let mut goals = vec![None; 2];
                        goals[waiter_idx] = Some(Goal::WaitOnPressurePlate(color, plate_pos));
                        goals[passer_idx] = Some(Goal::PassThroughDoor(color, last, target_pos));
                        return goals;
                    }

                    debug!(
                        "CoopPressurePlateDoorStrategy: Could not calculate valid target for P{} through door at {:?}",
                        passer_idx + 1,
                        door_pos
                    );
                    continue;
                }
            }
        }

        debug!("CoopPressurePlateDoorStrategy: No suitable plate/door combination found");
        vec![None; world.players.len()]
    }
}

impl SelectGoal for PickupHealthStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 10 || world.health.is_empty() {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over all health potions
        for health_pos in world.health.get_positions() {
            let mut best_player: Option<(usize, i32, usize)> = None; // (player_index, health, distance)

            // Find the best player for this specific health potion
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a health pickup in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Check if any enemy is close (within 2 tiles actual path distance)
                let enemy_nearby = world.enemies.get_positions().iter().any(|&enemy_pos| {
                    world.path_distance_to_enemy(player.position, enemy_pos) <= 2
                });

                if enemy_nearby {
                    continue;
                }

                // Check if this player can reach this health potion
                if let Some(path) = world.map.find_path(player.position, *health_pos) {
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
                    }
                }
            }

            // Assign this health potion to the best player found
            if let Some((player_index, _, _)) = best_player {
                debug!(
                    "[PickupHealthStrategy] Player {} selected for PickupHealth (health={}, pos={:?})",
                    player_index + 1,
                    world.players[player_index].health,
                    health_pos
                );
                goals[player_index] = Some(Goal::PickupHealth(*health_pos));
            }
        }

        goals
    }
}

impl SelectGoal for PickupSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 10 || world.swords.is_empty() {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over all swords
        for sword_pos in world.swords.get_positions() {
            let mut best_player: Option<(usize, usize)> = None; // (player_index, distance)

            // Find the best player for this specific sword
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a sword pickup in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already has a sword
                if player.has_sword {
                    continue;
                }

                // Check if this player can reach this sword
                if let Some(path) = world.map.find_path(player.position, *sword_pos) {
                    let distance = path.len();
                    let should_select = match best_player {
                        None => true,
                        Some((_, best_distance)) => {
                            // Prefer closer player
                            distance < best_distance
                        }
                    };

                    if should_select {
                        best_player = Some((player_index, distance));
                    }
                }
            }

            // Assign this sword to the best player found
            if let Some((player_index, _)) = best_player {
                debug!(
                    "[PickupSwordStrategy] Player {} selected for PickupSword (has_sword={}, sword_pos={:?})",
                    player_index + 1,
                    world.players[player_index].has_sword,
                    sword_pos
                );
                goals[player_index] = Some(Goal::PickupSword);
            }
        }

        goals
    }
}

impl SelectGoal for DropBoulderOnPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 6 {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Only consider this color if there's a door of the same color
            if !world.doors.has_color(color) {
                continue;
            }

            // Get pressure plates for this color
            let plates = match world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            debug!("Found {} {:?} pressure plates with matching doors", plates.len(), color);

            let mut best_player: Option<(usize, i32, Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a plate in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player is not carrying a boulder
                if player.inventory != crate::swoq_interface::Inventory::Boulder {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in plates {
                    if let Some(path_len) = world.path_distance(player.position, plate_pos)
                        && path_len < min_distance
                    {
                        min_distance = path_len;
                        closest_plate = Some(plate_pos);
                    }
                }

                // If we found at least one reachable plate, consider this player
                if let Some(plate_pos) = closest_plate {
                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance, plate_pos)),
                        Some((_, best_distance, _)) if min_distance < best_distance => {
                            Some((player_index, min_distance, plate_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[DropBoulderOnPlateStrategy] Player {} carrying boulder assigned to {:?} pressure plate at {:?} with matching door",
                    player_index + 1,
                    color,
                    plate_pos
                );
                goals[player_index] = Some(Goal::DropBoulderOnPlate(color, plate_pos));
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
        &mut self,
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

/// Helper struct to hold pathfinding results for a single player
struct PlayerReachability {
    can_reach_plate: bool,
    can_reach_door: bool,
    distance_to_plate: usize,
    path_to_door: Option<Vec<Position>>,
}

impl SelectGoal for CooperativeDoorPassageStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn prioritize(&self, world: &WorldState) -> bool {
        // In Execute state, prioritize if there's active cooperation
        if self.state == CooperativeDoorPassageState::Execute {
            self.has_active_door_cooperation(world)
        } else {
            false
        }
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        match self.state {
            CooperativeDoorPassageState::Setup => self.setup_phase(world, current_goals),
            CooperativeDoorPassageState::Execute => self.execute_phase(world),
        }
    }
}

impl SelectGoal for UsePressurePlateForDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Get door positions for this color
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Get pressure plates for this color
            let plates = match world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            // Find plates that are adjacent to doors of the same color
            let adjacent_plates: Vec<Position> = plates
                .iter()
                .copied()
                .filter(|plate_pos| {
                    plate_pos
                        .neighbors()
                        .iter()
                        .any(|neighbor| door_positions.contains(neighbor))
                })
                .collect();

            if adjacent_plates.is_empty() {
                continue;
            }

            debug!("Found {} {:?} pressure plates adjacent to doors", adjacent_plates.len(), color);

            let mut best_player: Option<(usize, i32, Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player has a key for this color (prefer keys over plates)
                if world.has_key(player, color) {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in &adjacent_plates {
                    let distance = if player.position.is_adjacent(&plate_pos) {
                        0 // Already adjacent
                    } else {
                        match world.map.find_path(player.position, plate_pos) {
                            Some(path) => path.len() as i32,
                            None => continue, // Can't reach this plate
                        }
                    };

                    if distance < min_distance {
                        min_distance = distance;
                        closest_plate = Some(plate_pos);
                    }
                }

                // If we found at least one reachable plate, consider this player
                if let Some(plate_pos) = closest_plate {
                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance, plate_pos)),
                        Some((_, best_dist, _)) if min_distance < best_dist => {
                            Some((player_index, min_distance, plate_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[UsePressurePlateForDoorStrategy] Player {} assigned to wait on {:?} pressure plate at {:?}",
                    player_index + 1,
                    color,
                    plate_pos
                );
                goals[player_index] = Some(Goal::WaitOnPressurePlate(color, plate_pos));
            }
        }

        goals
    }
}

impl SelectGoal for OpenDoorWithKeyStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Iterate over each door color
        for &color in world.doors.colors() {
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Check if any door of this color has a reachable empty neighbor from any player
            let mut best_player: Option<(usize, i32)> = None; // (player_index, distance)

            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player doesn't have the key for this color
                if !world.has_key(player, color) {
                    continue;
                }

                // Check if any door of this color is reachable by this player
                let mut min_distance = i32::MAX;
                for &door_pos in door_positions {
                    // Check if any neighbor of the door is reachable
                    for &neighbor in &door_pos.neighbors() {
                        // Only consider empty tiles (or player position)
                        if neighbor != player.position
                            && !matches!(
                                world.map.get(&neighbor),
                                Some(crate::swoq_interface::Tile::Empty)
                            )
                        {
                            continue;
                        }

                        // Calculate distance to this neighbor
                        let distance = if player.position == neighbor {
                            0 // Already at the door
                        } else {
                            match world.map.find_path(player.position, neighbor) {
                                Some(path) => path.len() as i32,
                                None => continue, // Can't reach this neighbor
                            }
                        };

                        min_distance = min_distance.min(distance);
                    }
                }

                // If we found at least one reachable door, consider this player
                if min_distance < i32::MAX {
                    debug!(
                        "Player {} can reach {:?} door (has key, distance: {})",
                        player_index + 1,
                        color,
                        min_distance
                    );

                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance)),
                        Some((_, best_dist)) if min_distance < best_dist => {
                            Some((player_index, min_distance))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _)) = best_player {
                debug!(
                    "[OpenDoorWithKeyStrategy] Player {} assigned to open {:?} door (has key, door reachable)",
                    player_index + 1,
                    color
                );
                goals[player_index] = Some(Goal::OpenDoor(color));
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
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Track which key colors have been assigned to prevent conflicts
        let mut assigned_key_colors = std::collections::HashSet::new();

        // In 2-player mode, check which door colors have reachable pressure plates
        let mut doors_with_plates = std::collections::HashSet::new();
        if world.is_two_player_mode() {
            for &color in &[Color::Red, Color::Green, Color::Blue] {
                if let Some(plates) = world.pressure_plates.get_positions(color) {
                    // Check if any player can reach any plate of this color
                    let can_reach_plate = world.players.iter().any(|player| {
                        plates.iter().any(|&plate_pos| {
                            world.map.find_path(player.position, plate_pos).is_some()
                        })
                    });
                    if can_reach_plate {
                        doors_with_plates.insert(color);
                        debug!("In 2-player mode: {:?} door has reachable pressure plate", color);
                    }
                }
            }
        }

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
                        
                        // In 2-player mode with reachable pressure plate, treat matching doors as walkable
                        let can_reach = if world.is_two_player_mode() && doors_with_plates.contains(&color) {
                            world.map.find_path_with_custom_walkability(player.position, key_pos, |pos, goal, _tick| {
                                // Check if this door matches our target color and we have a reachable plate
                                let is_matching_door = matches!((world.map.get(pos), color), 
                                        (Some(crate::swoq_interface::Tile::DoorRed), Color::Red) | 
                                        (Some(crate::swoq_interface::Tile::DoorGreen), Color::Green) | 
                                        (Some(crate::swoq_interface::Tile::DoorBlue), Color::Blue));
                                if is_matching_door {
                                    debug!("Treating {:?} door at {:?} as walkable (plate reachable in 2P mode)", color, pos);
                                    true
                                } else {
                                    world.map.is_walkable(pos, goal)
                                }
                            }).is_some()
                        } else {
                            world.map.find_path(player.position, key_pos).is_some()
                        };
                        
                        if can_reach {
                            debug!(
                                "[GetKeyForDoorStrategy] Player {} assigned to get {:?} key (reachable)",
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
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
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
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        if world.level < 6 || world.boulders.is_empty() {
            return goals;
        }

        // Track which boulders have been assigned to prevent conflicts
        let mut assigned_boulders = std::collections::HashSet::new();

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Only consider this color if there's a door and pressure plate
            if !world.doors.has_color(color) || !world.pressure_plates.has_color(color) {
                continue;
            }

            // Check if any door of this color is reachable by any player
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            debug!("Checking {:?} color for boulder fetch (has doors and plates)", color);

            let mut best_player: Option<(usize, i32, Position)> = None; // (player_index, distance, boulder_pos)

            // Find the best player to fetch a boulder for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal (from current goals or reused goal)
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }
                if player.inventory != crate::swoq_interface::Inventory::None {
                    continue;
                }

                // Check if this player can reach any door of this color
                let can_reach_door = door_positions.iter().any(|&door_pos| {
                    door_pos.neighbors().iter().any(|&neighbor| {
                        matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                            && world.map.find_path(player.position, neighbor).is_some()
                    })
                });

                if !can_reach_door {
                    continue;
                }

                // First, check if this player has an existing FetchBoulder goal that's still valid
                if let Some(Goal::FetchBoulder(boulder_pos)) = &player.previous_goal {
                    // Check if this boulder still exists and hasn't been assigned
                    if world.boulders.get_all_positions().contains(boulder_pos)
                        && !assigned_boulders.contains(boulder_pos)
                    {
                        // Verify the boulder is still reachable
                        let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                            world.map.is_walkable(&adj, adj)
                                && world.map.find_path(player.position, adj).is_some()
                        });

                        if can_reach {
                            debug!(
                                "[FetchBoulderForPlateStrategy] Player {} reusing existing FetchBoulder goal for boulder at {:?} (conditions still valid)",
                                player_index + 1,
                                boulder_pos
                            );
                            goals[player_index] = Some(Goal::FetchBoulder(*boulder_pos));
                            assigned_boulders.insert(*boulder_pos);
                            continue;
                        }
                    }
                }

                // Find the closest reachable boulder that hasn't been assigned
                let mut closest_boulder = None;
                let mut min_distance = i32::MAX;

                for boulder_pos in world.boulders.get_all_positions() {
                    // Skip if this boulder is already assigned
                    if assigned_boulders.contains(&boulder_pos) {
                        continue;
                    }

                    // Check if we can reach an adjacent position to pick it up
                    let can_reach = boulder_pos.neighbors().iter().any(|&adj| {
                        world.map.is_walkable(&adj, adj)
                            && world.map.find_path(player.position, adj).is_some()
                    });

                    if can_reach {
                        let dist = player.position.distance(&boulder_pos);
                        if dist < min_distance {
                            min_distance = dist;
                            closest_boulder = Some(boulder_pos);
                        }
                    }
                }

                // If we found a reachable boulder, consider this player
                if let Some(boulder_pos) = closest_boulder {
                    // Update best player if this one is closer to their boulder
                    best_player = match best_player {
                        None => Some((player_index, min_distance, boulder_pos)),
                        Some((_, best_distance, _)) if min_distance < best_distance => {
                            Some((player_index, min_distance, boulder_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _, boulder_pos)) = best_player {
                debug!(
                    "[FetchBoulderForPlateStrategy] Player {} assigned to fetch boulder at {:?} for {:?} pressure plate (has matching reachable door)",
                    player_index + 1,
                    boulder_pos,
                    color
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
        &mut self,
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
                            "[MoveUnexploredBoulderStrategy] Player {} assigned boulder at {:?}",
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
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Iterate over each color and assign one player per color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Get door positions for this color
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Get pressure plates for this color
            let plates = match world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            // Find plates that are within distance 4 of doors of the same color
            let nearby_plates: Vec<Position> = plates
                .iter()
                .copied()
                .filter(|plate_pos| {
                    door_positions
                        .iter()
                        .any(|door_pos| plate_pos.distance(door_pos) <= 4)
                })
                .collect();

            if nearby_plates.is_empty() {
                continue;
            }

            let mut best_player: Option<(usize, i32, Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // If nothing else to do and area is fully explored, step on any reachable pressure plate
                if !player.unexplored_frontier.is_empty() {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in &nearby_plates {
                    let distance = if player.position.is_adjacent(&plate_pos) {
                        0 // Already adjacent
                    } else {
                        match world.map.find_path(player.position, plate_pos) {
                            Some(path) => path.len() as i32,
                            None => continue, // Can't reach this plate
                        }
                    };

                    if distance < min_distance {
                        min_distance = distance;
                        closest_plate = Some(plate_pos);
                    }
                }

                // If we found at least one reachable plate, consider this player
                if let Some(plate_pos) = closest_plate {
                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance, plate_pos)),
                        Some((_, best_dist, _)) if min_distance < best_dist => {
                            Some((player_index, min_distance, plate_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player for this color
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[FallbackPressurePlateStrategy] Player {} assigned to {:?} pressure plate at {:?} (within distance 4 of door) as fallback",
                    player_index + 1,
                    color,
                    plate_pos
                );
                goals[player_index] = Some(Goal::WaitOnPressurePlate(color, plate_pos));
            }
        }

        goals
    }
}

impl SelectGoal for HuntEnemyWithSwordStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
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
        StrategyType::Individual
    }

    fn try_select(&mut self, world: &WorldState, player_index: usize) -> Option<Goal> {
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
