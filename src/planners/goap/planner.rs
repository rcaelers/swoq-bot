use crate::planners::goap::actions::*;
use crate::planners::goap::planner_state::PlannerState;
use crate::planners::goap::state_evaluator::evaluate_state;
use crate::state::WorldState;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

/// Node in the A* search tree for GOAP planning
/// Each node contains plans for ALL players in shared state
#[derive(Clone)]
#[allow(dead_code)]
struct PlanNode {
    /// Action sequences for each player (indexed by player_id)
    player_sequences: Vec<Vec<Box<dyn GOAPActionTrait>>>,

    /// End time for each player (when their current sequence completes)
    player_end_times: Vec<u32>,

    /// Simulated shared state after applying all actions
    state: PlannerState,

    /// Initial state for comparison (state evaluation)
    initial_state: PlannerState,

    /// Cumulative cost for all players (just action costs, no rewards)
    g_cost: f32,

    /// Heuristic cost (h_cost): estimated cost to goal
    h_cost: f32,

    /// Total depth (total number of actions across all players)
    total_actions: usize,
}

impl PlanNode {
    fn f_cost(&self) -> f32 {
        self.g_cost + self.h_cost
    }

    /// Get the next player to plan for (earliest end time)
    fn next_player_to_plan(&self, num_players: usize) -> Option<usize> {
        let mut earliest_player = None;
        let mut earliest_time = u32::MAX;

        for player_id in 0..num_players {
            if self.player_end_times[player_id] < earliest_time {
                earliest_time = self.player_end_times[player_id];
                earliest_player = Some(player_id);
            }
        }

        earliest_player
    }
}

impl PartialEq for PlanNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_cost() == other.f_cost()
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
        // Reverse ordering for min-heap (lower f_cost is better)
        other
            .f_cost()
            .partial_cmp(&self.f_cost())
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// GOAP Planner using A* search to find action sequences
pub struct GOAPPlanner {
    pub state: Option<PlannerState>,
    pub max_depth: usize,
    pub timeout: Duration,
}

impl GOAPPlanner {
    pub fn new(max_depth: usize, timeout_ms: u64) -> Self {
        Self {
            state: None,
            max_depth,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    /// Initialize or update planner state
    pub fn update_state(&mut self, world: &WorldState) {
        if let Some(ref mut state) = self.state {
            state.world = world.clone();
            state.sync_player_count();
        } else {
            self.state = Some(PlannerState::new(world.clone()));
        }
    }

    /// Check if replanning is needed
    /// Returns (needs_replan, is_emergency)
    pub fn needs_replan(&self) -> (bool, bool) {
        self.state
            .as_ref()
            .map_or((true, true), |s| s.needs_replan())
    }

    /// Plan actions for all players using A* search with shared state
    pub fn plan(&mut self, world: &WorldState) {
        let start_time = Instant::now();

        // Plan for all players simultaneously
        let plans = self.plan_all_players(world, start_time);

        let current_tick = world.tick as u32;

        // Apply plans to each player
        for (player_id, (plan_sequence, total_duration)) in plans.into_iter().enumerate() {
            if !plan_sequence.is_empty() {
                let start_tick = current_tick;
                let end_tick = start_tick + total_duration;

                tracing::info!(
                    "GOAP: Player {} plan: {} actions (start: {}, end: {})",
                    player_id,
                    plan_sequence.len(),
                    start_tick,
                    end_tick
                );
                for (i, action) in plan_sequence.iter().enumerate() {
                    tracing::info!("  [{}] {:?}", i, action);
                }

                let state = self.state.as_mut().unwrap();
                state.player_states[player_id].plan_sequence = plan_sequence;
                state.player_states[player_id].current_action_index = 0;
                state.player_states[player_id].action_start_time = Some(start_tick);
                state.player_states[player_id].action_end_time = Some(end_tick);
            } else {
                // No plan for this player
                self.state.as_mut().unwrap().clear_plan(player_id);
            }
        }
    }

    /// Plan for all players simultaneously using A* search
    /// Each expansion adds one action to the player with earliest end time
    #[tracing::instrument(skip(self, world, start_time))]
    fn plan_all_players(
        &mut self,
        world: &WorldState,
        start_time: Instant,
    ) -> Vec<(Vec<Box<dyn GOAPActionTrait>>, u32)> {
        let initial_state = self.state.as_ref().unwrap().clone();
        let num_players = world.players.len();
        let current_tick = world.tick as u32;

        tracing::debug!(
            current_tick = current_tick,
            num_players = num_players,
            "Starting plan_all_players"
        );

        // Create initial node (empty sequences for all players)
        let root_node = PlanNode {
            player_sequences: vec![Vec::new(); num_players],
            player_end_times: vec![current_tick; num_players],
            state: initial_state.clone(),
            initial_state: initial_state.clone(),
            g_cost: 0.0,
            h_cost: 0.0,
            total_actions: 0,
        };

        let mut open_set = BinaryHeap::new();
        open_set.push(root_node);

        let mut best_plan: Option<PlanNode> = None;
        let mut best_state_reward = f32::MIN;
        let mut best_g_cost = f32::MAX;

        // A* search - expand nodes until we reach max_depth total actions
        while let Some(current_node) = open_set.pop() {
            let all_plans: Vec<Vec<String>> = current_node
                .player_sequences
                .iter()
                .map(|seq| seq.iter().map(|a| a.name().to_string()).collect())
                .collect();
            tracing::debug!(
                total_actions = current_node.total_actions,
                g_cost = current_node.g_cost,
                score = -current_node.g_cost,
                player_end_times = ?current_node.player_end_times,
                all_plans = ?all_plans,
                "Exploring node"
            );

            // Check timeout
            if start_time.elapsed() > self.timeout {
                tracing::warn!(total_actions = current_node.total_actions, "Planning timeout");
                break;
            }

            // Determine which player to plan for next (earliest end time)
            let next_player = match current_node.next_player_to_plan(num_players) {
                Some(p) => p,
                None => continue,
            };

            // Skip inactive players
            if !world.players[next_player].is_active {
                tracing::debug!(player_id = next_player, "Skipping inactive player");
                continue;
            }

            let current_plan_length = current_node.player_sequences[next_player].len();
            let player_plan: Vec<String> = current_node.player_sequences[next_player]
                .iter()
                .map(|action| action.name().to_string())
                .collect();

            // // Calculate end times for each action in the plan
            // let mut action_end_times = Vec::new();
            // let mut time = current_tick;
            // for action in current_node.player_sequences[next_player].iter() {
            //     let duration = action.duration(&current_node.state, next_player);
            //     time += duration;
            //     action_end_times.push(time);
            // }

            // Prepare simulated state for this player's new action
            // The new action will start at next_player's current end time
            let action_start_time = current_node.player_end_times[next_player];
            let current_node_time = *current_node.player_end_times.iter().min().unwrap();

            tracing::debug!(
                player_id = next_player,
                end_time = current_node.player_end_times[next_player],
                current_plan_length = current_plan_length,
                position = ?(world.players[next_player].position.x, world.players[next_player].position.y),
                player_plan = ?player_plan,
                //action_end_times = ?action_end_times,
                action_start_time = action_start_time,
                current_node_time = current_node_time,
                time_delta = action_start_time - current_node_time,
                "Planning for player"
            );

            // Apply effect of the previous action of this player (if any)
            // The previous action completes at action_start_time (when the new action begins)
            let mut simulated_state = current_node.state.clone();
            let player_sequence = &current_node.player_sequences[next_player];
            if !player_sequence.is_empty() {
                let previous_action = player_sequence.last().unwrap();
                previous_action.effect(&mut simulated_state, next_player);
            }

            // If this node has actions, evaluate the simulated state for best plan tracking
            if current_node.total_actions > 0 {
                let state_reward = evaluate_state(&simulated_state, &current_node.initial_state);
                tracing::info!(
                    player_id = next_player,
                    total_actions = current_node.total_actions,
                    state_reward = state_reward,
                    g_cost = current_node.g_cost,
                    "Evaluated state"
                );

                // Select best plan by: (1) highest state_reward, (2) lowest g_cost (path length) as tiebreaker
                let is_better = if (state_reward - best_state_reward).abs() < 0.001 {
                    // State rewards are equal (within epsilon), use g_cost as tiebreaker
                    current_node.g_cost < best_g_cost
                } else {
                    state_reward > best_state_reward
                };

                if is_better {
                    tracing::info!(
                        total_actions = current_node.total_actions,
                        old_state_reward = best_state_reward,
                        new_state_reward = state_reward,
                        old_g_cost = best_g_cost,
                        new_g_cost = current_node.g_cost,
                        "Found better plan"
                    );
                    best_state_reward = state_reward;
                    best_g_cost = current_node.g_cost;
                    best_plan = Some(current_node.clone());
                }
            }

            // If we've reached max depth, don't expand further
            if current_node.total_actions >= self.max_depth {
                continue;
            }

            // Generate all candidate actions for this player using the simulated state
            // Each action's generate() function filters by preconditions internally
            let mut candidates: Vec<Box<dyn GOAPActionTrait>> = Vec::new();
            let explore_count = ExploreAction::generate(&simulated_state, next_player).len();
            candidates.extend(ExploreAction::generate(&simulated_state, next_player));
            let key_count = GetKeyAction::generate(&simulated_state, next_player).len();
            candidates.extend(GetKeyAction::generate(&simulated_state, next_player));
            let door_count = OpenDoorAction::generate(&simulated_state, next_player).len();
            candidates.extend(OpenDoorAction::generate(&simulated_state, next_player));
            let sword_count = PickupSwordAction::generate(&simulated_state, next_player).len();
            candidates.extend(PickupSwordAction::generate(&simulated_state, next_player));
            let attack_count = AttackEnemyAction::generate(&simulated_state, next_player).len();
            candidates.extend(AttackEnemyAction::generate(&simulated_state, next_player));
            let avoid_count = AvoidEnemyAction::generate(&simulated_state, next_player).len();
            candidates.extend(AvoidEnemyAction::generate(&simulated_state, next_player));
            let plate_door_count =
                PassThroughDoorWithPlateAction::generate(&simulated_state, next_player).len();
            // candidates
            //     .extend(PassThroughDoorWithPlateAction::generate(&simulated_state, next_player));
            let wait_count = WaitOnPlateAction::generate(&simulated_state, next_player).len();
            // candidates.extend(WaitOnPlateAction::generate(&simulated_state, next_player));
            let pickup_boulder_count =
                PickupBoulderAction::generate(&simulated_state, next_player).len();
            candidates.extend(PickupBoulderAction::generate(&simulated_state, next_player));
            let drop_boulder_count =
                DropBoulderAction::generate(&simulated_state, next_player).len();
            candidates.extend(DropBoulderAction::generate(&simulated_state, next_player));
            let drop_on_plate_count =
                DropBoulderOnPlateAction::generate(&simulated_state, next_player).len();
            candidates.extend(DropBoulderOnPlateAction::generate(&simulated_state, next_player));
            let touch_plate_count = TouchPlateAction::generate(&simulated_state, next_player).len();
            candidates.extend(TouchPlateAction::generate(&simulated_state, next_player));
            let exit_count = ReachExitAction::generate(&simulated_state, next_player).len();
            candidates.extend(ReachExitAction::generate(&simulated_state, next_player));

            if !candidates.is_empty() {
                tracing::debug!(
                    player_id = next_player,
                    total = candidates.len(),
                    explore = explore_count,
                    get_key = key_count,
                    open_door = door_count,
                    pickup_sword = sword_count,
                    attack = attack_count,
                    avoid = avoid_count,
                    plate_door = plate_door_count,
                    wait = wait_count,
                    pickup_boulder = pickup_boulder_count,
                    drop_boulder = drop_boulder_count,
                    drop_on_plate = drop_on_plate_count,
                    touch_plate = touch_plate_count,
                    exit = exit_count,
                    "Generated candidates"
                );
            } else {
                tracing::debug!(player_id = next_player, "No candidates generated");
            }

            // Expand node by trying each candidate action for this player
            // All candidates already passed precondition check during generation
            for (candidate_idx, action) in candidates.iter().enumerate() {
                let all_plans: Vec<Vec<String>> = current_node
                    .player_sequences
                    .iter()
                    .map(|seq| seq.iter().map(|a| a.name().to_string()).collect())
                    .collect();
                let duration = action.duration(&current_node.state, next_player);
                let action_end_time = action_start_time + duration;
                let cost = action.cost(&current_node.state, next_player);

                tracing::trace!(
                    player_id = next_player,
                    candidate = candidate_idx + 1,
                    total_candidates = candidates.len(),
                    action = ?action,
                    cost = cost,
                    duration = duration,
                    all_player_plans = ?all_plans,
                    current_node_time = current_node_time,
                    action_start_time = action_start_time,
                    action_end_time = action_end_time,
                    "Evaluating candidate"
                );

                let mut child_sequences = current_node.player_sequences.clone();
                child_sequences[next_player].push(action.clone());

                let mut child_end_times = current_node.player_end_times.clone();
                child_end_times[next_player] = action_end_time;

                let child_g_cost = current_node.g_cost + cost;

                tracing::trace!(
                    player_id = next_player,
                    action = ?action,
                    new_end_time = child_end_times[next_player],
                    child_g_cost = child_g_cost,
                    "Creating child node"
                );

                // If this is a terminal action, mark it as max depth to prevent further expansion
                let child_total_actions = if action.is_terminal() {
                    tracing::debug!(
                        player_id = next_player,
                        action = ?action,
                        "Terminal action added - marking node as terminal"
                    );
                    self.max_depth // Mark as terminal - won't be expanded further
                } else {
                    current_node.total_actions + 1
                };

                let child_node = PlanNode {
                    player_sequences: child_sequences,
                    player_end_times: child_end_times,
                    state: simulated_state.clone(),
                    initial_state: current_node.initial_state.clone(),
                    g_cost: child_g_cost,
                    h_cost: 0.0,
                    total_actions: child_total_actions,
                };
                open_set.push(child_node);
            }
        }

        // Return best plan found
        tracing::info!(
            best_state_reward = best_state_reward,
            best_g_cost = best_g_cost,
            plan_found = best_plan.is_some(),
            "A* search completed"
        );

        if let Some(plan) = best_plan {
            for (player_id, sequence) in plan.player_sequences.iter().enumerate() {
                if !sequence.is_empty() {
                    let action_names: Vec<String> =
                        sequence.iter().map(|a| format!("{:?}", a)).collect();
                    tracing::info!(
                        player_id = player_id,
                        sequence = ?action_names,
                        end_time = plan.player_end_times[player_id],
                        "Selected plan for player"
                    );
                }
            }

            // Convert to return format: Vec<(sequence, duration)>
            plan.player_sequences
                .into_iter()
                .enumerate()
                .map(|(player_id, sequence)| {
                    let duration = if sequence.is_empty() {
                        0
                    } else {
                        plan.player_end_times[player_id] - current_tick
                    };
                    (sequence, duration)
                })
                .collect()
        } else {
            tracing::warn!("No valid plan found, falling back to exploration");
            // Fallback: create exploration action for each player
            (0..num_players)
                .map(|player_id| {
                    let actions = ExploreAction::generate(&initial_state, player_id);
                    if let Some(action) = actions.into_iter().next() {
                        let duration = action.duration(&initial_state, player_id);
                        (vec![action], duration)
                    } else {
                        // If no exploration action can be generated, return empty plan
                        (vec![], 0)
                    }
                })
                .collect()
        }
    }
}
