use tracing::debug;

use crate::goals::Goal;
use crate::strategies::*;
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

pub trait SelectGoal {
    /// Returns the strategy type (Individual or Coop)
    fn strategy_type(&self) -> StrategyType;

    /// Return true if this is an emergency strategy that should always run first.
    /// Emergency strategies can override other goals (e.g., attack/flee from enemies).
    /// Default implementation returns false (not an emergency strategy).
    fn is_emergency(&self) -> bool {
        false
    }

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
                Box::new(attack_or_flee_enemy::AttackOrFleeEnemyStrategy),
                Box::new(pickup_health::PickupHealthStrategy),
                Box::new(pickup_sword::PickupSwordStrategy),
                Box::new(reach_exit::ReachExitStrategy),
                Box::new(boulder_on_plate::BoulderOnPlateStrategy::new()),
                Box::new(cooperative_door_passage::CooperativeDoorPassageStrategy::new()),
                Box::new(use_pressure_plate_for_door::UsePressurePlateForDoorStrategy),
                Box::new(key_and_door::KeyAndDoorStrategy::new()),
                Box::new(move_unexplored_boulder::MoveUnexploredBoulderStrategy),
                Box::new(fallback_pressure_plate::FallbackPressurePlateStrategy),
                Box::new(hunt_enemy_with_sword::HuntEnemyWithSwordStrategy::new()),
                Box::new(random_explore::RandomExploreStrategy),
            ],
            last_strategy_per_player: Vec::new(),
        }
    }

    /// Find a random reachable position for forced random exploration
    fn find_random_reachable_position(
        world: &WorldState,
        player_index: usize,
    ) -> Option<crate::types::Position> {
        let player = &world.players[player_index];

        // Collect all empty positions that we've seen
        let empty_positions: Vec<crate::types::Position> = world
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
            return None;
        }

        // Try random positions until we find a reachable one (max 10 attempts)
        let mut seed = world.tick as usize;

        for _ in 0..10 {
            let index = seed % empty_positions.len();
            let target = empty_positions[index];

            // Check if reachable
            if world.find_path(player.position, target).is_some() {
                debug!("Forced RandomExplore: Selected reachable position {:?}", target);
                return Some(target);
            }

            // Try next position
            seed = seed.wrapping_add(1);
        }

        debug!("Forced RandomExplore: No reachable position found after 10 attempts");
        None
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

        // Check for players forced into random exploration due to oscillation
        for player_index in 0..num_players {
            if world.players[player_index].force_random_explore_ticks > 0 {
                debug!(
                    "Player {} forced to RandomExplore (remaining ticks: {})",
                    player_index + 1,
                    world.players[player_index].force_random_explore_ticks
                );

                if let Some(target) = Self::find_random_reachable_position(world, player_index) {
                    selected_goals[player_index] = Some(Goal::RandomExplore(target));
                }
            }
        }

        // First, process emergency strategies (e.g., attack/flee enemies)
        // These can override any other goals
        for (strategy_idx, strategy) in self.strategies.iter_mut().enumerate() {
            if strategy.is_emergency() {
                debug!("Processing emergency strategy {}", strategy_idx);
                Self::process_strategy(
                    strategy,
                    strategy_idx,
                    world,
                    &mut selected_goals,
                    &mut current_strategy_per_player,
                    num_players,
                    false,
                );
            }
        }

        // Second, try to prioritize strategies that were used last tick
        let unique_last_strategies: std::collections::HashSet<usize> = self
            .last_strategy_per_player
            .iter()
            .filter_map(|&s| s)
            .collect();

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

        // Process remaining strategies in order (skip players with forced random exploration)
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
                            debug!(
                                "Player {} re-selected goal from prioritized strategy: {:?}",
                                player_index + 1,
                                goal
                            );
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
                                debug!(
                                    "Player {} selected co-op goal from prioritized strategy: {:?}",
                                    player_index + 1,
                                    goal
                                );
                            } else {
                                debug!(
                                    "Player {} selected co-op goal: {:?}",
                                    player_index + 1,
                                    goal
                                );
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
