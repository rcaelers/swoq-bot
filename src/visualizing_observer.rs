use std::sync::{Arc, Mutex, mpsc};

use crate::game_observer::GameObserver;
use crate::goal::Goal;
use crate::swoq_interface::{DirectedAction, GameStatus, State};
use crate::visualizer::{LogColor, LogMessage};
use crate::world_state::WorldState;

pub struct VisualizingObserver {
    shared_state: Arc<Mutex<Option<WorldState>>>,
    log_tx: mpsc::Sender<LogMessage>,
}

impl VisualizingObserver {
    pub fn new(
        shared_state: Arc<Mutex<Option<WorldState>>>,
        log_tx: mpsc::Sender<LogMessage>,
    ) -> Self {
        Self {
            shared_state,
            log_tx,
        }
    }

    fn send_log(&self, text: String, color: LogColor) {
        let _ = self.log_tx.send(LogMessage { text, color });
    }

    fn update_shared_state(&self, world: &WorldState) {
        if let Ok(mut state) = self.shared_state.lock() {
            *state = Some(world.clone());
            tracing::debug!(
                "Updated shared state: {} tiles at level {} tick {}",
                world.map.len(),
                world.level,
                world.tick
            );
        } else {
            tracing::warn!("Failed to lock shared state for update");
        }
    }
}

impl GameObserver for VisualizingObserver {
    fn on_game_start(
        &mut self,
        game_id: &str,
        seed: Option<i32>,
        map_width: i32,
        map_height: i32,
        visibility_range: i32,
    ) {
        tracing::info!(
            "Game started: id={}, seed={:?}, size={}x{}, visibility={}",
            game_id,
            seed,
            map_width,
            map_height,
            visibility_range
        );
    }

    fn on_new_level(&mut self, level: i32) {
        self.send_log(format!("New Level: {}", level), LogColor::White);
    }

    fn on_state_update(&mut self, _state: &State, world: &WorldState) {
        self.send_log(format!("TICK {}", world.tick), LogColor::Cyan);
        self.update_shared_state(world);

        tracing::debug!(
            "Level {}, Tick {}: Health={}, Position=({}, {})",
            world.level,
            world.tick,
            world.player().health,
            world.player().position.x,
            world.player().position.y
        );

        // Draw the ASCII map to stdout to preserve colors
        println!("\n{}", "─".repeat(60));
        
        // Line 1: Level and tick (left), Player 1 info (right)
        let p1 = world.player();
        let p1_goal = p1.previous_goal
            .as_ref()
            .map(|g| format!("{:?}", g))
            .unwrap_or_else(|| "None".to_string());
        
        let p1_inv = if p1.has_sword {
            format!("Sword+{:?}", p1.inventory)
        } else {
            format!("{:?}", p1.inventory)
        };
        
        let p1_info = format!(
            "P1  HP:{:<3}  Inv:{:<16}  Goal:{}",
            p1.health,
            p1_inv,
            p1_goal
        );
        let left_side = format!("Level:{:<4} Tick:{:<6}", world.level, world.tick);
        let total_width: usize = 60;
        let spacing = total_width.saturating_sub(left_side.len() + p1_info.len());
        println!("{}{}{}", left_side, " ".repeat(spacing), p1_info);
        
        // Line 2: Player 2 info (right aligned) if available
        if world.players.len() > 1 {
            let p2 = &world.players[1];
            let p2_goal = p2.previous_goal
                .as_ref()
                .map(|g| format!("{:?}", g))
                .unwrap_or_else(|| "None".to_string());
            
            let p2_inv = if p2.has_sword {
                format!("Sword+{:?}", p2.inventory)
            } else {
                format!("{:?}", p2.inventory)
            };
            
            let p2_info = format!(
                "P2  HP:{:<3}  Inv:{:<16}  Goal:{}",
                p2.health,
                p2_inv,
                p2_goal
            );
            println!("{:>60}", p2_info);
        }
        
        let map = world.draw_ascii_map();
        println!("{}", map);
        println!("{}", "─".repeat(60));
    }

    fn on_goal_selected(&mut self, goal: &Goal, _world: &WorldState) {
        tracing::debug!("Selected Goal: {:?}", goal);
        self.send_log(format!("Selected Goal: {:?}", goal), LogColor::Green);
    }

    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState) {
        self.send_log(format!("Executing Action: {:?}", action), LogColor::Yellow);

        tracing::debug!(
            "Action selected: {:?} at ({}, {})",
            action,
            world.player().position.x,
            world.player().position.y
        );
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        tracing::info!("Game finished: status={:?}, tick={}", status, final_tick);
    }
}
