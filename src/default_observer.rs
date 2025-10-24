use crate::game_observer::GameObserver;
use crate::goal::Goal;
use crate::swoq_interface::{DirectedAction, GameStatus, State};
use crate::world_state::WorldState;
use std::io::{self, Write};
use tracing::info;

pub struct DefaultObserver;

impl GameObserver for DefaultObserver {
    fn on_game_start(
        &mut self,
        game_id: &str,
        seed: Option<i32>,
        map_width: i32,
        map_height: i32,
        visibility_range: i32,
    ) {
        info!("Game {} started", game_id);
        if let Some(seed) = seed {
            info!("- seed: {}", seed);
        }
        info!("- map size: {}x{}", map_height, map_width);
        info!("- visibility range: {}", visibility_range);
    }

    fn on_new_level(&mut self, level: i32) {
        info!("Level changed to {}", level);
    }

    fn on_state_update(&mut self, state: &State, world: &WorldState) {
        info!(
            "tick: {}, pos: ({}, {}), health: {}",
            state.tick, world.player_pos.x, world.player_pos.y, world.player_health,
        );

        let map = world.draw_ascii_map();
        let _ = writeln!(io::stdout(), "{}", map);

        let _ = write!(io::stdout(), "Inventory: {:?}", world.player_inventory);
        if world.player_has_sword {
            let _ = write!(io::stdout(), " [Has Sword]");
        }
        let _ = writeln!(io::stdout(), " | Health: {}", world.player_health);
    }

    fn on_goal_selected(&mut self, goal: &Goal, _world: &WorldState) {
        info!("Selected Goal: {:?}", goal);
    }

    fn on_action_selected(&mut self, action: DirectedAction, _world: &WorldState) {
        info!("action: {}", action.as_str_name());
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        info!("\nGame finished with status: {:?}", status);
        info!("Final tick: {}", final_tick);
    }
}
