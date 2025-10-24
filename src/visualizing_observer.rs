use crate::game_observer::GameObserver;
use crate::swoq_interface::{DirectedAction, GameStatus, State};
use crate::world_state::WorldState;
use std::sync::{Arc, Mutex};

pub struct VisualizingObserver {
    shared_state: Arc<Mutex<Option<WorldState>>>,
}

impl VisualizingObserver {
    pub fn new(shared_state: Arc<Mutex<Option<WorldState>>>) -> Self {
        Self { shared_state }
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

    fn on_new_level(&mut self, level: i32, previous_level: i32) {
        tracing::info!("New level: {} (from {})", level, previous_level);
    }

    fn on_state_update(&mut self, _state: &State, world: &WorldState) {
        // Update the shared state for the visualizer
        self.update_shared_state(world);

        tracing::debug!(
            "Level {}, Tick {}: Health={}, Position=({}, {})",
            world.level,
            world.tick,
            world.player_health,
            world.player_pos.x,
            world.player_pos.y
        );

        // Draw the ASCII map to stdout to preserve colors
        println!("\n{}", "─".repeat(60));
        let map = world.draw_ascii_map();
        println!("{}", map);
        println!("{}", "─".repeat(60));
    }

    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState) {
        tracing::debug!(
            "Action selected: {:?} at ({}, {})",
            action,
            world.player_pos.x,
            world.player_pos.y
        );
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        tracing::info!("Game finished: status={:?}, tick={}", status, final_tick);
    }
}
