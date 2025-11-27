use crate::planners::goap::actions::*;
use crate::planners::goap::game_state::PlanningState;
use crate::planners::goap::state_evaluator::evaluate_state;
use crate::state::WorldState;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

pub type PlayerPlan = Vec<Box<dyn GOAPActionTrait>>;
pub type Plan = Vec<PlayerPlan>;

// Epsilon for floating-point comparison in plan evaluation
const REWARD_COMPARISON_EPSILON: f32 = 0.001;

/// Node in the A* search tree for GOAP planning
/// Each node contains plans for ALL players in shared state
#[derive(Clone)]
struct PlanNode {
    /// Action sequences for each player (indexed by player_id)
    player_sequences: Plan,

    /// End time for each player (when their current sequence completes)
    player_end_times: Vec<u32>,

    /// Last time point that was fully processed (all players at this time expanded)
    last_processed_time: u32,

    /// Player that expanded this node
    #[allow(dead_code)]
    player: Option<usize>,

    /// World state before the most recently added action was applied
    world_before_last_action: WorldState,

    /// World state after the most recently added action (computed lazily)
    world_after_last_action: Option<WorldState>,

    /// State before the most recently added action was applied
    state_before_last_action: PlanningState,

    /// State after the most recently added action (computed lazily)
    state_after_last_action: Option<PlanningState>,

    /// Initial world state for comparison (state evaluation)
    initial_world: WorldState,

    /// Initial state for comparison (state evaluation)
    initial_state: PlanningState,

    /// Cumulative cost for all players (just action costs, no rewards)
    cost: f32,

    /// Accumulated action rewards
    action_rewards: f32,
}

impl PlanNode {
    fn cost(&self) -> f32 {
        self.cost
    }

    /// Get all players at the earliest end time after last_processed_time
    fn get_idle_players(&self, num_players: usize) -> Vec<usize> {
        // Find the earliest time at or after last_processed_time
        let mut earliest_time: Option<u32> = None;
        for player_id in 0..num_players {
            let player_time = self.player_end_times[player_id];
            tracing::trace!(
                player_id = player_id,
                player_time = player_time,
                last_processed_time = self.last_processed_time,
                "Checking player end time"
            );
            if player_time >= self.last_processed_time {
                earliest_time = Some(match earliest_time {
                    None => player_time,
                    Some(current) => current.min(player_time),
                });
            }
        }

        // Return all players at that earliest time
        let Some(earliest) = earliest_time else {
            return Vec::new();
        };

        (0..num_players)
            .filter(|&player_id| self.player_end_times[player_id] == earliest)
            .collect()
    }

    fn all_plans(&self) -> Vec<Vec<String>> {
        self.player_sequences
            .iter()
            .map(|seq| seq.iter().map(|a| a.name().to_string()).collect())
            .collect()
    }

    fn plan_for_player(&self, idle_player: usize) -> Vec<String> {
        self.player_sequences[idle_player]
            .iter()
            .map(|action| action.name().to_string())
            .collect()
    }

    fn total_actions(&self) -> usize {
        self.player_sequences.iter().map(|seq| seq.len()).sum()
    }

    fn is_player_terminal(&self, player_id: usize) -> bool {
        if let Some(last_action) = self.player_sequences[player_id].last() {
            last_action.is_terminal()
        } else {
            false
        }
    }

    fn update_end_state(&mut self, player_id: usize) {
        // Start from existing end state if available, otherwise from before state
        let mut simulated_world = self
            .world_after_last_action
            .clone()
            .unwrap_or_else(|| self.world_before_last_action.clone());
        let mut simulated_state = self
            .state_after_last_action
            .clone()
            .unwrap_or_else(|| self.state_before_last_action.clone());

        let player_sequence = &self.player_sequences[player_id];
        tracing::trace!(
            player_id = player_id,
            player = ?self.player,
            action_sequence = ?player_sequence.iter().map(|a| a.name()).collect::<Vec<_>>(),
            "Simulating end state for player"
        );
        if !player_sequence.is_empty() {
            tracing::trace!(
                player_id = player_id,
                "applying action {}",
                player_sequence.last().unwrap().name()
            );
            let previous_action = player_sequence.last().unwrap();
            previous_action.effect_end(&mut simulated_world, &mut simulated_state, player_id);
        }
        self.world_after_last_action = Some(simulated_world);
        self.state_after_last_action = Some(simulated_state);
        self.player = Some(player_id);
    }
}

impl PartialEq for PlanNode {
    fn eq(&self, other: &Self) -> bool {
        self.cost() == other.cost()
    }
}

impl Eq for PlanNode {}

impl PartialOrd for PlanNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PlanNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering for min-heap (lower cost is better)
        other
            .cost()
            .partial_cmp(&self.cost())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// GOAP Planner using A* search to find action sequences
pub struct Planner {
    pub max_depth: usize,
    pub timeout: Duration,

    // A* search state
    open_set: BinaryHeap<PlanNode>,
    best_plan: Option<PlanNode>,
    best_state_reward: f32,
    best_cost: f32,
}

impl Planner {
    pub fn new(max_depth: usize, timeout_ms: u64) -> Self {
        Self {
            max_depth,
            timeout: Duration::from_millis(timeout_ms),
            open_set: BinaryHeap::new(),
            best_plan: None,
            best_state_reward: f32::MIN,
            best_cost: f32::MAX,
        }
    }

    fn evaluate(&mut self, node: &PlanNode) {
        if node.total_actions() > 0 {
            // For evaluation, we need to apply ALL actions (including those beyond last_processed_time)
            // Start from world_after_last_action which has actions up to last_processed_time applied
            let mut eval_world = node
                .world_after_last_action
                .as_ref()
                .unwrap_or(&node.world_before_last_action)
                .clone();
            let mut eval_state = node
                .state_after_last_action
                .as_ref()
                .unwrap_or(&node.state_before_last_action)
                .clone();

            // There's at most one action that needs effect_end applied: the last action for one player
            // that starts after last_processed_time. Find which player has an action at last_processed_time.
            for (player_id, sequence) in node.player_sequences.iter().enumerate() {
                if !sequence.is_empty()
                    && node.player_end_times[player_id] > node.last_processed_time
                {
                    // This player's last action starts after last_processed_time
                    let last_action = sequence.last().unwrap();
                    tracing::trace!(
                        player_id = player_id,
                        action = last_action.name(),
                        "Applying effect_end of last action for evaluation"
                    );
                    last_action.effect_end(&mut eval_world, &mut eval_state, player_id);
                }
            }

            let state_reward =
                evaluate_state(&eval_world, &eval_state, &node.initial_world, &node.initial_state);

            // Skip invalid plans (NEG_INFINITY) - don't store them as best_plan
            if state_reward.is_infinite() && state_reward.is_sign_negative() {
                tracing::debug!(
                    total_actions = node.total_actions(),
                    "Skipping invalid plan (NEG_INFINITY reward)"
                );
                return;
            }

            let total_reward = state_reward + node.action_rewards;
            tracing::info!(
                total_actions = node.total_actions(),
                state_reward = state_reward,
                action_rewards = node.action_rewards,
                total_reward = total_reward,
                cost = node.cost,
                "Evaluated state"
            );

            // Log player positions and destinations
            for (player_id, player) in eval_world.players.iter().enumerate() {
                tracing::info!(
                    player_id = player_id,

                    position = ?(player.position.x, player.position.y),
                    destination = ?player.current_destination.map(|d| (d.x, d.y)),
                    "Player state"
                );
            }

            // Select best plan by: (1) highest total_reward, (2) lowest cost as tiebreaker
            let is_better = self.best_plan.is_none()
                || if (total_reward - self.best_state_reward).abs() < REWARD_COMPARISON_EPSILON {
                    // Total rewards are equal (within epsilon), use cost as tiebreaker
                    node.cost < self.best_cost
                } else {
                    total_reward > self.best_state_reward
                };

            if is_better {
                tracing::info!(
                    total_actions = node.total_actions(),
                    old_total_reward = self.best_state_reward,
                    new_total_reward = total_reward,
                    old_cost = self.best_cost,
                    new_cost = node.cost,
                    "Found better plan"
                );
                self.best_state_reward = total_reward;
                self.best_cost = node.cost;
                self.best_plan = Some(node.clone());
            }
        }
    }

    /// Calculate wait duration for a player to synchronize with other players.
    /// Returns Some(duration) if waiting makes sense, None otherwise.
    fn calculate_wait_duration(current_node: &PlanNode, player_index: usize) -> Option<u32> {
        if player_index >= current_node.player_end_times.len() {
            return None;
        }

        // Check if any other player's last action is a Wait
        // If so, don't allow this player to also wait (prevents deadlock)
        for (other_player_idx, sequence) in current_node.player_sequences.iter().enumerate() {
            if other_player_idx != player_index && !sequence.is_empty() {
                let last_action_name = sequence.last().unwrap().name();
                if last_action_name.starts_with("Wait") {
                    return None;
                }
            }
        }

        let current_player_time = current_node.player_end_times[player_index];

        // Find earliest time among OTHER players (including those without actions yet)
        // Players without actions are idle at their end_time and may get actions in other branches
        let other_players_earliest_time = current_node
            .player_end_times
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != player_index)
            .map(|(_, &time)| time)
            .min();

        if let Some(earliest_other_time) = other_players_earliest_time {
            // Don't wait if other player is in terminal state (u32::MAX)
            if earliest_other_time == u32::MAX {
                return None;
            }

            if earliest_other_time >= current_player_time {
                // Wait until 1 tick after the other player becomes idle
                let wait_duration = (earliest_other_time - current_player_time) + 1;
                return Some(wait_duration);
            }
        }

        None
    }

    fn generate_candidates(
        &self,
        current_node: &PlanNode,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let simulated_world = current_node.world_after_last_action.as_ref().unwrap();
        let simulated_state = current_node.state_after_last_action.as_ref().unwrap();
        let mut candidates: Vec<Box<dyn GOAPActionTrait>> = Vec::new();

        let explore_actions = ExploreAction::generate(simulated_world, simulated_state, player_index);
        let explore_count = explore_actions.len();
        candidates.extend(explore_actions);

        let key_actions = GetKeyAction::generate(simulated_world, simulated_state, player_index);
        let key_count = key_actions.len();
        candidates.extend(key_actions);

        let door_actions = OpenDoorAction::generate(simulated_world, simulated_state, player_index);
        let door_count = door_actions.len();
        candidates.extend(door_actions);

        let sword_actions = PickupSwordAction::generate(simulated_world, simulated_state, player_index);
        let sword_count = sword_actions.len();
        candidates.extend(sword_actions);

        let health_actions = PickupHealthAction::generate(simulated_world, simulated_state, player_index);
        let health_count = health_actions.len();
        candidates.extend(health_actions);

        let attack_actions = AttackEnemyAction::generate(simulated_world, simulated_state, player_index);
        let attack_count = attack_actions.len();
        candidates.extend(attack_actions);

        let hunt_actions = HuntEnemyAction::generate(simulated_world, simulated_state, player_index);
        let hunt_count = hunt_actions.len();
        candidates.extend(hunt_actions);

        let avoid_actions = AvoidEnemyAction::generate(simulated_world, simulated_state, player_index);
        let avoid_count = avoid_actions.len();
        candidates.extend(avoid_actions);

        let plate_door_actions =
            PassThroughDoorWithPlateAction::generate(simulated_world, simulated_state, player_index);
        let plate_door_count = plate_door_actions.len();
        // candidates.extend(plate_door_actions);

        let wait_actions = WaitOnPlateAction::generate(simulated_world, simulated_state, player_index);
        let wait_count = wait_actions.len();
        // candidates.extend(wait_actions);

        let pickup_boulder_actions = PickupBoulderAction::generate(simulated_world, simulated_state, player_index);
        let pickup_boulder_count = pickup_boulder_actions.len();
        candidates.extend(pickup_boulder_actions);

        let drop_boulder_actions = DropBoulderAction::generate(simulated_world, simulated_state, player_index);
        let drop_boulder_count = drop_boulder_actions.len();
        candidates.extend(drop_boulder_actions);

        let drop_on_plate_actions =
            DropBoulderOnPlateAction::generate(simulated_world, simulated_state, player_index);
        let drop_on_plate_count = drop_on_plate_actions.len();
        candidates.extend(drop_on_plate_actions);

        let touch_plate_actions = TouchPlateAction::generate(simulated_world, simulated_state, player_index);
        let touch_plate_count = touch_plate_actions.len();
        candidates.extend(touch_plate_actions);

        let exit_actions = ReachExitAction::generate(simulated_world, simulated_state, player_index);
        let exit_count = exit_actions.len();
        candidates.extend(exit_actions);

        // Generate WaitAction with context from current_node
        let wait_action_count = if let Some(wait_duration) =
            Self::calculate_wait_duration(current_node, player_index)
        {
            candidates.push(Box::new(WaitAction::new(wait_duration)));
            1
        } else {
            0
        };

        if !candidates.is_empty() {
            tracing::debug!(
                player_id = player_index,
                total = candidates.len(),
                explore = explore_count,
                get_key = key_count,
                open_door = door_count,
                pickup_sword = sword_count,
                pickup_health = health_count,
                attack = attack_count,
                hunt = hunt_count,
                avoid = avoid_count,
                plate_door = plate_door_count,
                wait_on_plate = wait_count,
                wait_action = wait_action_count,
                pickup_boulder = pickup_boulder_count,
                drop_boulder = drop_boulder_count,
                drop_on_plate = drop_on_plate_count,
                touch_plate = touch_plate_count,
                exit = exit_count,
                "Generated candidates"
            );
        } else {
            tracing::debug!(player_id = player_index, "No candidates generated");
        }

        candidates
    }

    fn generate_child_nodes(
        &mut self,
        candidates: Vec<Box<dyn GOAPActionTrait>>,
        current_node: &PlanNode,
        idle_player: usize,
    ) {
        for (candidate_idx, action) in candidates.iter().enumerate() {
            let action_start_time = current_node.player_end_times[idle_player];
            let current_node_time = *current_node.player_end_times.iter().min().unwrap();

            let duration = action.duration(&current_node.world_before_last_action, &current_node.state_before_last_action, idle_player);
            let action_end_time = if action.is_terminal() {
                u32::MAX
            } else {
                action_start_time + duration
            };

            let cost = action.cost(&current_node.world_before_last_action, &current_node.state_before_last_action, idle_player);
            let child_cost = current_node.cost + cost;

            let action_reward = action.reward(&current_node.world_before_last_action, &current_node.state_before_last_action, idle_player);
            let child_action_rewards = current_node.action_rewards + action_reward;

            let mut child_sequences = current_node.player_sequences.clone();
            child_sequences[idle_player].push(action.clone());

            let mut child_end_times = current_node.player_end_times.clone();
            child_end_times[idle_player] = action_end_time;

            // Create child state and apply effect_start to claim resources
            let mut child_world = current_node
                .world_after_last_action
                .as_ref()
                .unwrap()
                .clone();
            let mut child_state = current_node
                .state_after_last_action
                .as_ref()
                .unwrap()
                .clone();
            action.effect_start(&mut child_world, &mut child_state, idle_player);

            tracing::trace!(
                player_id = idle_player,
                candidate = candidate_idx + 1,
                total_candidates = candidates.len(),
                action = ?action,
                cost = cost,
                duration = duration,
                all_player_plans = ?current_node.all_plans(),
                current_node_time = current_node_time,
                action_start_time = action_start_time,
                action_end_time = action_end_time,
                child_cost = child_cost,
                action_reward = action_reward,
                child_action_rewards = child_action_rewards,
                child_sequences = ?child_sequences.iter().map(|seq| seq.iter().map(|a| a.name()).collect::<Vec<_>>()).collect::<Vec<_>>(),
                child_end_times = ?child_end_times,
                "Queueing child node"
            );

            let child_node = PlanNode {
                player_sequences: child_sequences,
                player_end_times: child_end_times,
                last_processed_time: action_start_time,
                world_before_last_action: child_world,
                world_after_last_action: None,
                state_before_last_action: child_state,
                state_after_last_action: None,
                initial_world: current_node.initial_world.clone(),
                initial_state: current_node.initial_state.clone(),
                cost: child_cost,
                action_rewards: child_action_rewards,
                player: None,
            };
            self.open_set.push(child_node);
        }
    }

    #[tracing::instrument(skip(self, world))]
    pub fn plan(mut self, world: &WorldState) -> Plan {
        let num_players = world.players.len();
        let current_tick = world.tick as u32;
        let start_time = Instant::now();
        let game_state = PlanningState::new(world);

        tracing::debug!(
            current_tick = current_tick,
            num_players = num_players,
            "Starting plan_all_players"
        );

        let root_node = PlanNode {
            player_sequences: vec![Vec::new(); num_players],
            player_end_times: vec![current_tick; num_players],
            last_processed_time: 0,
            world_before_last_action: world.clone(),
            world_after_last_action: None,
            state_before_last_action: game_state.clone(),
            state_after_last_action: None,
            initial_world: world.clone(),
            initial_state: game_state.clone(),
            player: None,
            cost: 0.0,
            action_rewards: 0.0,
        };

        self.open_set.push(root_node);

        while let Some(mut current_node) = self.open_set.pop() {
            tracing::debug!(
                total_actions = current_node.total_actions(),
                cost = current_node.cost,
                score = -current_node.cost,
                player_end_times = ?current_node.player_end_times,
                all_plans = ?current_node.all_plans(),
                "Exploring node"
            );

            if start_time.elapsed() > self.timeout {
                tracing::warn!(total_actions = current_node.total_actions(), "Planning timeout");
                break;
            }

            let idle_players = current_node.get_idle_players(num_players);

            if idle_players.is_empty() {
                tracing::debug!("All players have completed their plans");
                continue;
            }

            let action_start_time = current_node.player_end_times[idle_players[0]];
            let current_node_time = *current_node.player_end_times.iter().min().unwrap();

            tracing::debug!(
                idle_players = ?idle_players,
                action_start_time = action_start_time,
                current_node_time = current_node_time,
                all_player_plans = ?current_node.all_plans(),
                "Processing players at time point"
            );

            // Update end state for ALL idle players at this time point to get complete state
            for &idle_player in &idle_players {
                current_node.update_end_state(idle_player);
            }

            // Always evaluate the state after updating all idle players
            // This ensures terminal states (like all players at exit) are evaluated
            self.evaluate(&current_node);

            // If we've reached max depth, don't expand further
            if current_node.total_actions() >= self.max_depth {
                tracing::debug!(
                    total_actions = current_node.total_actions(),
                    "Reached max depth, not expanding further"
                );
                continue;
            }

            // Now try to expand ONE player (trying all players in order until we find candidates)
            // Start with idle players, then try other players if needed
            let mut candidates = Vec::new();
            let mut selected_player = None;

            // First, try idle players (already have their end state updated)
            for &player_id in &idle_players {
                if current_node.is_player_terminal(player_id) {
                    tracing::debug!(
                        player_id = player_id,
                        "Player has reached terminal action, skipping"
                    );
                    continue;
                }

                tracing::debug!(
                    player_id = player_id,
                    end_time = current_node.player_end_times[player_id],
                    position = ?(world.players[player_id].position.x, world.players[player_id].position.y),
                    player_plan = ?current_node.plan_for_player(player_id),
                    "Trying to generate candidates for idle player"
                );

                candidates = self.generate_candidates(&current_node, player_id);

                if !candidates.is_empty() {
                    selected_player = Some(player_id);
                    tracing::debug!(
                        player_id = player_id,
                        candidate_count = candidates.len(),
                        "Found candidates for idle player"
                    );
                    break;
                }
            }

            // If no idle player has candidates, try all other players
            if selected_player.is_none() {
                for player_id in 0..num_players {
                    // Skip if already tried (was in idle_players)
                    if idle_players.contains(&player_id) {
                        continue;
                    }

                    if current_node.is_player_terminal(player_id) {
                        tracing::debug!(
                            player_id = player_id,
                            "Player has reached terminal action, skipping"
                        );
                        continue;
                    }

                    tracing::debug!(
                        player_id = player_id,
                        end_time = current_node.player_end_times[player_id],
                        position = ?(world.players[player_id].position.x, world.players[player_id].position.y),
                        player_plan = ?current_node.plan_for_player(player_id),
                        "Trying to generate candidates for non-idle player"
                    );

                    // Update this player's end state before generating candidates
                    current_node.update_end_state(player_id);

                    // Re-evaluate after updating this player's state
                    self.evaluate(&current_node);

                    candidates = self.generate_candidates(&current_node, player_id);

                    if !candidates.is_empty() {
                        selected_player = Some(player_id);
                        tracing::debug!(
                            player_id = player_id,
                            candidate_count = candidates.len(),
                            "Found candidates for non-idle player"
                        );
                        break;
                    }
                }
            }

            if let Some(player_id) = selected_player {
                self.generate_child_nodes(candidates, &current_node, player_id);
            } else {
                tracing::debug!("No candidates generated for any player, state already evaluated");
            }
        }

        // Return best plan found
        tracing::info!(
            best_state_reward = self.best_state_reward,
            best_cost = self.best_cost,
            plan_found = self.best_plan.is_some(),
            "A* search completed"
        );

        if let Some(plan) = self.best_plan.as_ref() {
            plan.player_sequences.clone()
        } else {
            Vec::new()
        }
    }
}
