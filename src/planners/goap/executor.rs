use crate::planners::goap::actions::{ActionExecutionState, ExecutionStatus};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

/// Executes GOAP plans for all players
pub struct GOAPExecutor;

impl GOAPExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Execute current plans for all players
    /// Returns Some(actions) if there are executable actions, None if plans exhausted
    pub fn execute(&self, state: &mut PlannerState, world: &mut WorldState) -> Option<Vec<DirectedAction>> {
        let mut actions = Vec::new();
        let mut has_executable_action = false;

        for player_id in 0..world.players.len() {
            if !world.players[player_id].is_active {
                actions.push(DirectedAction::None);
                continue;
            }

            // Execute player's plan - loop to handle multiple completions in one tick
            let mut final_action = DirectedAction::None;
            let mut found_executable_action = false;

            loop {
                let player_state = &state.player_states[player_id];
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

                let (action, status) = current_action.execute(
                    world,
                    player_id,
                    &mut state.player_states[player_id].execution_state,
                );

                // Check if action is complete or failed
                match status {
                    ExecutionStatus::Complete => {
                        tracing::info!(
                            "GOAP: Player {} completed action [{}/{}]: {:?}",
                            player_id,
                            state.player_states[player_id].current_action_index + 1,
                            state.player_states[player_id].plan_sequence.len(),
                            current_action
                        );

                        // Move to next action in sequence
                        state.player_states[player_id].current_action_index += 1;
                        state.player_states[player_id].execution_state =
                            ActionExecutionState::default();

                        // If we've completed all actions in the sequence, clear the plan
                        if state.player_states[player_id].current_action_index
                            >= state.player_states[player_id].plan_sequence.len()
                        {
                            state.clear_plan(player_id);
                        }

                        // If action completed with None, continue to next action
                        // Otherwise, send this action and stop
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
                            state.player_states[player_id].current_action_index + 1,
                            state.player_states[player_id].plan_sequence.len(),
                            current_action
                        );
                        state.clear_plan(player_id);

                        // Failed action - don't send to server
                        break;
                    }
                    ExecutionStatus::InProgress => {
                        // Continue executing - send action to server
                        final_action = action;
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
}
