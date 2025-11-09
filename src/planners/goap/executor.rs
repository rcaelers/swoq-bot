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
    pub fn execute(&self, state: &mut PlannerState, world: &mut WorldState) -> Vec<DirectedAction> {
        let mut actions = Vec::new();

        for player_id in 0..world.players.len() {
            if !world.players[player_id].is_active {
                actions.push(DirectedAction::None);
                continue;
            }

            // Execute player's plan
            let player_state = &state.player_states[player_id];
            if !player_state.plan_sequence.is_empty()
                && player_state.current_action_index < player_state.plan_sequence.len()
            {
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
                    }
                    ExecutionStatus::InProgress => {
                        // Continue executing
                    }
                }

                actions.push(action);
            } else {
                // No plan or plan exhausted
                if player_state.plan_sequence.is_empty() {
                    tracing::debug!("GOAP: Player {} has no plan", player_id);
                }
                actions.push(DirectedAction::None);
            }
        }

        actions
    }
}
