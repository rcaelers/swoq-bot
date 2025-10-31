use std::io::{self, Write};
use tracing::info;

use crate::game_observer::GameObserver;
use crate::goals::Goal;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};
use crate::world_state::WorldState;

#[derive(Default)]
pub struct DefaultObserver {
    game_count: i32,
}


impl GameObserver for DefaultObserver {
    fn on_game_start(
        &mut self,
        game_id: &str,
        seed: Option<i32>,
        map_width: i32,
        map_height: i32,
        visibility_range: i32,
    ) {
        self.game_count += 1;
        info!(
            "Game #{} started: id={}, seed={:?}, size={}x{}, visibility={}",
            self.game_count, game_id, seed, map_width, map_height, visibility_range
        );
    }

    fn on_new_level(&mut self, level: i32) {
        info!("Level changed to {}", level);
    }

    fn on_state_update(&mut self, state: &State, world: &WorldState) {
        let p1 = &world.players[0];
        tracing::debug!(
            "Game #{}, Level {}, Tick {}: P1 Health={}, Position=({}, {})",
            self.game_count,
            world.level,
            state.tick,
            p1.health,
            p1.position.x,
            p1.position.y
        );

        if world.players.len() > 1 {
            let p2 = &world.players[1];
            tracing::debug!(
                "                    P2 Health={}, Position=({}, {})",
                p2.health,
                p2.position.x,
                p2.position.y
            );
        }

        // Print player 1 surroundings
        if let Some(player_state) = &state.player_state {
            let surroundings = world.draw_surroundings(&player_state.surroundings, p1.position, 1);
            let _ = writeln!(io::stdout(), "{}", surroundings);
        }

        // Print player 2 surroundings
        if world.players.len() > 1
            && let Some(player2_state) = &state.player2_state
        {
            let p2 = &world.players[1];
            let surroundings = world.draw_surroundings(&player2_state.surroundings, p2.position, 2);
            let _ = writeln!(io::stdout(), "{}", surroundings);
        }

        let map = world.draw_ascii_map();
        let _ = writeln!(io::stdout(), "{}", map);

        let _ = write!(io::stdout(), "P1 Inventory: {:?}", p1.inventory);
        if p1.has_sword {
            let _ = write!(io::stdout(), " [Has Sword]");
        }
        let _ = write!(io::stdout(), " | Health: {}", p1.health);

        if world.players.len() > 1 {
            let p2 = &world.players[1];
            let _ = write!(io::stdout(), "  P2 Inventory: {:?}", p2.inventory);
            if p2.has_sword {
                let _ = write!(io::stdout(), " [Has Sword]");
            }
            let _ = write!(io::stdout(), " | Health: {}", p2.health);
        }
        let _ = writeln!(io::stdout());
    }

    fn on_goal_selected(&mut self, goal: &Goal, _world: &WorldState) {
        tracing::debug!("Selected Goal: {:?}", goal);
    }

    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState) {
        let p1 = &world.players[0];
        tracing::debug!("Action selected: {:?} at ({}, {})", action, p1.position.x, p1.position.y);
    }

    fn on_action_result(
        &mut self,
        action: DirectedAction,
        action2: Option<DirectedAction>,
        result: ActResult,
        _world: &WorldState,
    ) {
        tracing::debug!("Action result: {:?}/{:?} -> {:?}", action, action2, result);
    }

    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32) {
        tracing::info!(
            "Game #{} finished: status={:?}, tick={}",
            self.game_count,
            status,
            final_tick
        );
    }
}
