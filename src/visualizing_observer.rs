use std::sync::{Arc, Mutex, mpsc};

use crate::game_observer::GameObserver;
use crate::goals::Goal;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};
use crate::visualizer::{LogColor, LogMessage};
use crate::world_state::WorldState;

pub struct VisualizingObserver {
    shared_state: Arc<Mutex<Option<WorldState>>>,
    log_tx: mpsc::Sender<LogMessage>,
    successful_runs: i32,
    failed_runs: i32,
    game_count: i32,
    last_action: Option<DirectedAction>,
    last_action2: Option<DirectedAction>,
    last_result: Option<ActResult>,
    last_p1_goal: Option<Goal>,
    last_p2_goal: Option<Goal>,
}

impl VisualizingObserver {
    pub fn new(
        shared_state: Arc<Mutex<Option<WorldState>>>,
        log_tx: mpsc::Sender<LogMessage>,
    ) -> Self {
        Self {
            shared_state,
            log_tx,
            successful_runs: 0,
            failed_runs: 0,
            game_count: 0,
            last_action: None,
            last_action2: None,
            last_result: None,
            last_p1_goal: None,
            last_p2_goal: None,
        }
    }

    fn send_log(&self, text: String, color: LogColor) {
        let _ = self.log_tx.send(LogMessage { text, color });
    }

    fn update_shared_state(&mut self, world: &WorldState) {
        if let Ok(mut state) = self.shared_state.lock() {
            let mut updated_world = world.clone();
            // Inject the persistent run counters
            updated_world.successful_runs = self.successful_runs;
            updated_world.failed_runs = self.failed_runs;
            updated_world.game_count = self.game_count;
            *state = Some(updated_world);
        }
    }
}

impl GameObserver for VisualizingObserver {
    fn on_game_start(
        &mut self,
        _game_id: &str,
        _seed: Option<i32>,
        _map_width: i32,
        _map_height: i32,
        _visibility_range: i32,
    ) {
        self.game_count += 1;
        self.last_p1_goal = None;
        self.last_p2_goal = None;
    }

    fn on_new_level(&mut self, level: i32) {
        self.send_log(format!("New Level: {}", level), LogColor::White);
    }

    fn on_state_update(&mut self, _state: &State, world: &WorldState) {
        self.update_shared_state(world);
    }

    fn on_goal_selected(&mut self, goal: &Goal, world: &WorldState) {
        // Track the goal - if we've already seen P1's goal, this must be P2
        if self.last_p1_goal.is_none() || world.players.len() == 1 {
            self.last_p1_goal = Some(goal.clone());
        } else {
            self.last_p2_goal = Some(goal.clone());
        }
    }

    fn on_action_selected(&mut self, _action: DirectedAction, _world: &WorldState) {
        // No logging
    }

    fn on_action_result(
        &mut self,
        action: DirectedAction,
        action2: Option<DirectedAction>,
        result: ActResult,
        _world: &WorldState,
    ) {
        self.last_action = Some(action);
        self.last_action2 = action2;
        self.last_result = Some(result);
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        // Update run statistics in the observer (persistent across game runs)
        match status {
            GameStatus::FinishedSuccess => {
                self.successful_runs += 1;
                // No logging for success
            }
            _ => {
                self.failed_runs += 1;

                // Get player positions from shared state
                let (p1_pos, p2_pos) = if let Ok(state) = self.shared_state.lock() {
                    if let Some(world) = state.as_ref() {
                        let p1 = world.players.first().map(|p| (p.position.x, p.position.y));
                        let p2 = world.players.get(1).map(|p| (p.position.x, p.position.y));
                        (p1, p2)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                // Build failure message with last action and result
                let mut message = format!(
                    "❌ FAILED Game #{}: status={:?}, tick={}",
                    self.game_count, status, final_tick
                );

                if let Some((x, y)) = p1_pos {
                    message.push_str(&format!("\n   P1 Position: ({}, {})", x, y));
                }

                if let Some(goal) = &self.last_p1_goal {
                    message.push_str(&format!("\n   P1 Goal: {:?}", goal));
                }

                if let Some((x, y)) = p2_pos {
                    message.push_str(&format!("\n   P2 Position: ({}, {})", x, y));
                }

                if let Some(goal) = &self.last_p2_goal {
                    message.push_str(&format!("\n   P2 Goal: {:?}", goal));
                }

                if let Some(action) = &self.last_action {
                    if let Some(action2) = &self.last_action2 {
                        message
                            .push_str(&format!("\n   Last action: {:?} + {:?}", action, action2));
                    } else {
                        message.push_str(&format!("\n   Last action: {:?}", action));
                    }
                }

                if let Some(result) = &self.last_result {
                    message.push_str(&format!("\n   Result: {}", result.as_str_name()));
                }

                self.send_log(message, LogColor::Red);
            }
        }
    }
}
