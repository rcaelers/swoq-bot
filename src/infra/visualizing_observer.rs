use std::sync::{Arc, Mutex, mpsc};

use crate::infra::GameObserver;
use crate::state::WorldState;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};
use crate::ui::{GameStateSnapshot, LogColor, LogMessage};

pub struct VisualizingObserver {
    shared_state: Arc<Mutex<Option<GameStateSnapshot>>>,
    log_tx: mpsc::Sender<LogMessage>,
    last_action: Option<DirectedAction>,
    last_action2: Option<DirectedAction>,
    last_result: Option<ActResult>,
    last_p1_goal: String,
    last_p2_goal: Option<String>,
    player_paths: Vec<Option<Vec<crate::infra::Position>>>,
    // Store statistics for shared state update
    game_count: i32,
    successful_runs: i32,
    failed_runs: i32,
}

impl VisualizingObserver {
    pub fn new(
        shared_state: Arc<Mutex<Option<GameStateSnapshot>>>,
        log_tx: mpsc::Sender<LogMessage>,
    ) -> Self {
        Self {
            shared_state,
            log_tx,
            last_action: None,
            last_action2: None,
            last_result: None,
            last_p1_goal: String::new(),
            last_p2_goal: None,
            player_paths: Vec::new(),
            game_count: 0,
            successful_runs: 0,
            failed_runs: 0,
        }
    }

    fn send_log(&self, text: String, color: LogColor) {
        let _ = self.log_tx.send(LogMessage { text, color });
    }

    fn update_shared_state(&mut self, world: &WorldState) {
        if let Ok(mut state) = self.shared_state.lock() {
            *state = Some(GameStateSnapshot {
                world: world.clone(),
                game_count: self.game_count,
                successful_runs: self.successful_runs,
                failed_runs: self.failed_runs,
                p1_goal: self.last_p1_goal.clone(),
                p2_goal: self.last_p2_goal.clone(),
                player_paths: self.player_paths.clone(),
            });
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
        self.last_p1_goal = String::new();
        self.last_p2_goal = None;
    }

    fn on_new_level(&mut self, level: i32) {
        self.send_log(format!("New Level: {}", level), LogColor::White);
    }

    fn on_state_update(
        &mut self,
        _state: &State,
        world: &WorldState,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    ) {
        // Update local statistics for shared state
        self.game_count = game_count;
        self.successful_runs = successful_runs;
        self.failed_runs = failed_runs;
        self.update_shared_state(world);
    }

    fn on_goal_selected(&mut self, player_index: usize, goal_name: &str, _world: &WorldState) {
        if player_index == 0 {
            self.last_p1_goal = goal_name.to_string();
        } else if player_index == 1 {
            self.last_p2_goal = Some(goal_name.to_string());
        }
    }

    fn on_paths_updated(&mut self, paths: Vec<Option<Vec<crate::infra::Position>>>) {
        self.player_paths = paths;
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

    fn on_game_finished(
        &mut self,
        status: GameStatus,
        final_tick: i32,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    ) {
        // Update local statistics
        self.game_count = game_count;
        self.successful_runs = successful_runs;
        self.failed_runs = failed_runs;

        // Only log failures
        if status != GameStatus::FinishedSuccess {
            // Get player positions from shared state
            let (p1_pos, p2_pos) = if let Ok(state) = self.shared_state.lock() {
                if let Some(snapshot) = state.as_ref() {
                    let p1 = snapshot
                        .world
                        .players
                        .first()
                        .map(|p| (p.position.x, p.position.y));
                    let p2 = snapshot
                        .world
                        .players
                        .get(1)
                        .map(|p| (p.position.x, p.position.y));
                    (p1, p2)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }; // Build failure message with last action and result
            let mut message =
                format!("❌ FAILED Game #{}: status={:?}, tick={}", game_count, status, final_tick);

            if let Some((x, y)) = p1_pos {
                message.push_str(&format!("\n   P1 Position: ({}, {})", x, y));
            }

            if !self.last_p1_goal.is_empty() {
                message.push_str(&format!("\n   P1 Goal: {}", self.last_p1_goal));
            }

            if let Some((x, y)) = p2_pos {
                message.push_str(&format!("\n   P2 Position: ({}, {})", x, y));
            }

            if let Some(goal) = &self.last_p2_goal {
                message.push_str(&format!("\n   P2 Goal: {}", goal));
            }

            if let Some(action) = &self.last_action {
                if let Some(action2) = &self.last_action2 {
                    message.push_str(&format!("\n   Last action: {:?} + {:?}", action, action2));
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

    fn on_oscillation_detected(&mut self, message: &str) {
        self.send_log(message.to_string(), LogColor::Yellow);
    }
}
