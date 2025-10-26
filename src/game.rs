use std::time::Instant;

use crate::game_observer::GameObserver;
use crate::strategy::Planner;
use crate::swoq::GameConnection;
use crate::swoq_interface::{self, DirectedAction, GameStatus};
use crate::world_state::WorldState;

pub struct Game {
    connection: GameConnection,
    observer: Box<dyn GameObserver>,
}

impl Game {
    pub fn new(connection: GameConnection, observer: impl GameObserver + 'static) -> Self {
        Self {
            connection,
            observer: Box::new(observer),
        }
    }

    pub async fn run(
        &mut self,
        level: Option<i32>,
        seed: Option<i32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut game = self.connection.start(level, seed).await?;

        self.observer.on_game_start(
            &game.game_id,
            game.seed,
            game.map_width,
            game.map_height,
            game.visibility_range,
        );

        let mut world = WorldState::new(game.map_width, game.map_height, game.visibility_range);
        let mut current_level = game.state.level;

        while game.state.status == swoq_interface::GameStatus::Active as i32 {
            let tick_start = Instant::now();

            if game.state.level != current_level {
                self.observer.on_new_level(game.state.level);
                world.reset_for_new_level();
                current_level = game.state.level;
            }

            println!("\n┌────────────────────────────────────────────────────────────┐");
            println!(
                "│ 📊 STATE UPDATE - Tick {}                                  ",
                game.state.tick
            );
            println!("└────────────────────────────────────────────────────────────┘");

            world.update(&game.state);
            self.observer.on_state_update(&game.state, &world);

            let player_actions = Planner::decide_action(&mut world);

            // Notify observer about goals and actions for all players
            for (idx, (goal, action)) in player_actions.iter().enumerate() {
                let player = &world.players[idx];
                tracing::debug!("Player {}: Selected Goal: {:?}", idx + 1, goal);
                tracing::debug!(
                    "Player {}: Action selected: {:?} at ({}, {})",
                    idx + 1,
                    action,
                    player.position.x,
                    player.position.y
                );

                // Notify observer for all players
                self.observer.on_goal_selected(goal, &world);
                if idx == 0 {
                    self.observer.on_action_selected(*action, &world);
                }
            }

            // Extract actions for game.act()
            let action1 = player_actions
                .first()
                .map(|(_, a)| *a)
                .unwrap_or(DirectedAction::None);
            let action2 = if world.players.len() > 1 {
                player_actions.get(1).map(|(_, a)| Some(*a)).unwrap_or(None)
            } else {
                None
            };

            let act_result = game.act(action1, action2).await?;

            // Notify observer about action result
            self.observer
                .on_action_result(action1, action2, act_result, &world);

            // Stop the game if action returned an error
            if act_result != swoq_interface::ActResult::Ok {
                println!("\n❌ Action failed with result: {:?}", act_result);
                println!("🛑 Stopping game due to action error");
                break;
            }

            let tick_duration = tick_start.elapsed();
            if tick_duration.as_millis() > 100 {
                println!(
                    "⚠️  Tick {} took {:.2}ms (level {}, actions: {:?} {:?}, result: {:?})",
                    game.state.tick,
                    tick_duration.as_secs_f64() * 1000.0,
                    game.state.level,
                    action1,
                    action2,
                    act_result
                );
            }
        }

        let status =
            GameStatus::try_from(game.state.status).unwrap_or(GameStatus::FinishedCanceled);
        self.observer.on_game_finished(status, game.state.tick);

        // Wait 5 seconds before continuing
        // println!("\n⏳ Game finished, waiting 5 seconds before continuing...");
        // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        Ok(())
    }
}
