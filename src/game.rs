use crate::game_observer::GameObserver;
use crate::strategy::Planner;
use crate::swoq::GameConnection;
use crate::swoq_interface::{self, GameStatus};
use crate::world_state::WorldState;
use std::time::Instant;

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

            let (goal, action) = Planner::decide_action(&mut world);
            self.observer.on_goal_selected(&goal, &world);
            self.observer.on_action_selected(action, &world);

            game.act(action).await?;

            let tick_duration = tick_start.elapsed();
            if tick_duration.as_millis() > 100 {
                println!(
                    "⚠️  Tick {} took {:.2}ms (level {}, action: {:?})",
                    game.state.tick,
                    tick_duration.as_secs_f64() * 1000.0,
                    game.state.level,
                    action
                );
            }
        }

        let status =
            GameStatus::try_from(game.state.status).unwrap_or(GameStatus::FinishedCanceled);
        self.observer.on_game_finished(status, game.state.tick);

        Ok(())
    }
}
