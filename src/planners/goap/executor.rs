use crate::planners::goap::actions::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

/// Per-player GOAP planning state
#[derive(Debug, Clone)]
pub struct PlayerExecutionState {
    /// Current action plan sequence for this player (up to max_depth actions)
    pub plan_sequence: Vec<Box<dyn GOAPActionTrait>>,

    /// Index of the current action being executed in the plan sequence
    pub current_action_index: usize,

    /// Execution state for tracking multi-tick actions
    pub execution_state: ActionExecutionState,
}

impl PlayerExecutionState {
    pub fn new(plan: Vec<Box<dyn GOAPActionTrait>>) -> Self {
        Self {
            plan_sequence: plan,
            current_action_index: 0,
            execution_state: ActionExecutionState::default(),
        }
    }
}

pub struct Executor {
    pub player_states: Vec<PlayerExecutionState>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            player_states: Vec::new(),
        }
    }

    pub fn step(&mut self, world: &mut WorldState) -> Option<Vec<DirectedAction>> {
        // Phase 1: Prepare - all actions set their destinations
        for (player_id, player_state) in self.player_states.iter_mut().enumerate() {
            if !world.players[player_id].is_active {
                continue;
            }

            if player_state.plan_sequence.is_empty()
                || player_state.current_action_index >= player_state.plan_sequence.len()
            {
                continue;
            }

            let current_action = &mut player_state.plan_sequence[player_state.current_action_index];
            let destination = current_action.prepare(world, player_id);
            world.players[player_id].current_destination = destination;
        }

        // Phase 2: CBS - compute collision-free paths for all players
        world.compute_cbs_paths();

        // Phase 3: Execute - all actions use CBS paths
        let mut actions = Vec::new();
        let mut has_executable_action = false;

        for (player_id, player_state) in self.player_states.iter_mut().enumerate() {
            if !world.players[player_id].is_active {
                actions.push(DirectedAction::None);
                continue;
            }

            tracing::debug!(
                "GOAP: Player {} location {:?} destination {} path {}",
                player_id,
                world.players[player_id].position,
                match &world.players[player_id].current_destination {
                    Some(d) => format!("{:?}", d),
                    None => "None".to_string(),
                },
                match &world.players[player_id].current_path {
                    Some(p) => format!("{:?}", p),
                    None => "None".to_string(),
                }
            );

            let mut final_action = DirectedAction::None;
            let mut found_executable_action = false;

            loop {
                if player_state.plan_sequence.is_empty()
                    || player_state.current_action_index >= player_state.plan_sequence.len()
                {
                    // No plan or plan exhausted
                    if player_state.plan_sequence.is_empty() {
                        tracing::debug!("GOAP: Player {} has no plan", player_id);
                    }
                    break;
                }

                let current_action =
                    player_state.plan_sequence[player_state.current_action_index].clone();

                let (action, status) =
                    current_action.execute(world, player_id, &mut player_state.execution_state);

                match status {
                    ExecutionStatus::Complete => {
                        tracing::info!(
                            "GOAP: Player {} completed action [{}/{}]: {:?}",
                            player_id,
                            player_state.current_action_index + 1,
                            player_state.plan_sequence.len(),
                            current_action
                        );

                        // Clear cached path and destination on completion
                        world.players[player_id].current_path = None;
                        world.players[player_id].current_destination = None;

                        player_state.current_action_index += 1;
                        player_state.execution_state = ActionExecutionState::default();

                        if matches!(action, DirectedAction::None) {
                            continue; // Try next action in plan
                        } else {
                            final_action = action;
                            found_executable_action = true;
                            break;
                        }
                    }

                    ExecutionStatus::Failed => {
                        tracing::warn!(
                            "GOAP: Player {} action failed [{}/{}]: {:?}",
                            player_id,
                            player_state.current_action_index + 1,
                            player_state.plan_sequence.len(),
                            current_action
                        );
                        // Clear cached path and destination on failure
                        world.players[player_id].current_path = None;
                        world.players[player_id].current_destination = None;
                        // Failed action - don't send to server
                        break;
                    }

                    ExecutionStatus::InProgress => {
                        // Continue executing - send action to server
                        final_action = action;
                        found_executable_action = true;
                        break;
                    }

                    ExecutionStatus::Wait => {
                        tracing::debug!(
                            "GOAP: Player {} action waiting for precondition [{}/{}]: {:?}",
                            player_id,
                            player_state.current_action_index + 1,
                            player_state.plan_sequence.len(),
                            current_action
                        );
                        // Wait for precondition to become true
                        final_action = DirectedAction::None;
                        found_executable_action = true;
                        break;
                    }
                }
            }

            if found_executable_action {
                has_executable_action = true;
            }

            actions.push(final_action);
        }

        // Return None if no player has an executable action (all plans exhausted)
        if has_executable_action {
            Some(actions)
        } else {
            None
        }
    }

    pub fn current_goal_names(&self) -> Vec<String> {
        self.player_states
            .iter()
            .map(|ps| {
                if ps.current_action_index < ps.plan_sequence.len() {
                    ps.plan_sequence[ps.current_action_index].name()
                } else {
                    String::new()
                }
            })
            .collect()
    }

    pub fn set_plans(&mut self, plans: Vec<Vec<Box<dyn GOAPActionTrait>>>) {
        for (player_id, plan) in plans.iter().enumerate() {
            tracing::info!(
                "GOAP: Player {} new plan ({} actions): {:?}",
                player_id,
                plan.len(),
                plan.iter().map(|a| a.name()).collect::<Vec<_>>()
            );
        }
        self.player_states = plans.into_iter().map(PlayerExecutionState::new).collect();
    }

    pub fn needs_replan(&self, world: &WorldState) -> (bool, bool) {
        // Only replan when all plans are complete (empty)
        // ExploreAction will mark itself complete when new objects are discovered
        let plan_complete = self
            .player_states
            .iter()
            .all(|ps| ps.current_action_index >= ps.plan_sequence.len());

        tracing::info!("Plan complete: {}", plan_complete);

        // Check for emergency: enemy too close
        // BUT skip emergency check if player is currently executing HuntEnemy action
        let mut is_emergency = false;
        for (player_id, player_state) in self.player_states.iter().enumerate() {
            let player = &world.players[player_id];
            tracing::debug!("Checking replan for player {} at position {:?}", player_id, player.position);
            if !player.is_active {
                continue;
            }

            // Skip emergency check if currently engaged in combat
            tracing::debug!("Player {} current plan: {:?}", player_id, player_state.plan_sequence);
            if player_state.current_action_index < player_state.plan_sequence.len() {
                tracing::debug!(
                    "Player {} current action index: {}",
                    player_id,
                    player_state.current_action_index
                );
                let current_action = &player_state.plan_sequence[player_state.current_action_index];
                tracing::debug!("Player {} current action: {}", player_id, current_action.name());
                if current_action.is_combat_action() {
                    tracing::debug!(
                        "Player {} is engaged in combat ({}), skipping emergency check",
                        player_id,
                        current_action.name()
                    );
                    continue;
                }
            }

            tracing::debug!("Player {} is active with health {}", player_id, player.health);
            let has_sword = player.has_sword;
            let danger_threshold = if has_sword { 2 } else { 3 };

            // Check distance to any enemy
            for enemy_pos in world.enemies.get_positions() {
                tracing::debug!(
                    "Checking distance from Player {} at {:?} to Enemy at {:?}",
                    player_id,
                    player.position,
                    enemy_pos
                );
                let dist = world.path_distance_to_enemy(player.position, *enemy_pos);
                tracing::debug!(
                    "Distance from Player {} to Enemy at {:?} is {} (threshold: {})",
                    player_id,
                    enemy_pos,
                    dist,
                    danger_threshold
                );
                if dist <= danger_threshold {
                    tracing::warn!(
                        "Emergency replan: Player {} too close to enemy at {:?} (distance: {}, threshold: {})",
                        player_id,
                        enemy_pos,
                        dist,
                        danger_threshold
                    );
                    is_emergency = true;
                    break;
                }
            }

            if is_emergency {
                break;
            }
        }

        (plan_complete || is_emergency, is_emergency)
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
