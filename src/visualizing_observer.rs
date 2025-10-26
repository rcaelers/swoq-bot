use std::sync::{Arc, Mutex, mpsc};

use crate::game_observer::GameObserver;
use crate::goal::Goal;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};
use crate::visualizer::{LogColor, LogMessage};
use crate::world_state::WorldState;

pub struct VisualizingObserver {
    shared_state: Arc<Mutex<Option<WorldState>>>,
    log_tx: mpsc::Sender<LogMessage>,
    successful_runs: i32,
    failed_runs: i32,
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
    }

    fn on_new_level(&mut self, level: i32) {
        self.send_log(format!("New Level: {}", level), LogColor::White);
    }

    fn on_state_update(&mut self, _state: &State, world: &WorldState) {
        self.send_log(format!("TICK {}", world.tick), LogColor::Cyan);
        self.update_shared_state(world);
    }

    fn on_goal_selected(&mut self, goal: &Goal, _world: &WorldState) {
        self.send_log(format!("Selected Goal: {:?}", goal), LogColor::Green);
    }

    fn on_action_selected(&mut self, action: DirectedAction, _world: &WorldState) {
        self.send_log(format!("Executing Action: {:?}", action), LogColor::Yellow);
    }

    fn on_action_result(
        &mut self,
        action: DirectedAction,
        action2: Option<DirectedAction>,
        result: ActResult,
        _world: &WorldState,
    ) {
        let color = if result == ActResult::Ok {
            LogColor::Green
        } else {
            LogColor::Red
        };

        let message = if let Some(action2) = action2 {
            format!("Result: {:?} + {:?} -> {}", action, action2, result.as_str_name())
        } else {
            format!("Result: {:?} -> {}", action, result.as_str_name())
        };

        self.send_log(message, color);
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        // Update run statistics in the observer (persistent across game runs)
        match status {
            GameStatus::FinishedSuccess => {
                self.successful_runs += 1;
                self.send_log(
                    format!("✅ Game finished SUCCESSFULLY (tick {})", final_tick),
                    LogColor::Green,
                );
            }
            _ => {
                self.failed_runs += 1;
                self.send_log(
                    format!("❌ Game finished: status={:?}, tick={}", status, final_tick),
                    LogColor::Red,
                );
            }
        }
    }
}
