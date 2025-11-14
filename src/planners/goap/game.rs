use std::time::Instant;

use crate::infra::{GameConnection, GameObserver};
use crate::planners::goap::{Executor, Planner};
use crate::state::WorldState;
use crate::swoq_interface::{self, DirectedAction, GameStatus};

pub struct Game {
    connection: GameConnection,
    observer: Box<dyn GameObserver>,
    world: WorldState,
    current_level: i32,
    executor: Executor,

    // Planner configuration
    planner_max_depth: usize,
    planner_timeout_ms: u64,

    // Game statistics (persistent across levels)
    pub successful_runs: i32,
    pub failed_runs: i32,
    pub game_count: i32,
}

impl Game {
    pub fn new(
        connection: GameConnection,
        observer: impl GameObserver + 'static,
        goap_max_depth: usize,
    ) -> Self {
        Self {
            connection,
            observer: Box::new(observer),
            world: WorldState::new(0, 0, 0),
            current_level: 0,
            planner_max_depth: goap_max_depth,
            planner_timeout_ms: 500,
            executor: Executor::new(),
            successful_runs: 0,
            failed_runs: 0,
            game_count: 0,
        }
    }

    pub async fn run(
        &mut self,
        level: Option<i32>,
        seed: Option<i32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut game = self.connection.start(level, seed).await?;

        self.game_count += 1;

        self.observer.on_game_start(
            &game.game_id,
            game.seed,
            game.map_width,
            game.map_height,
            game.visibility_range,
        );

        // Reset world state for new game
        self.world = WorldState::new(game.map_width, game.map_height, game.visibility_range);
        self.current_level = game.state.level;

        // Reset executor for new game
        self.executor = Executor::new();

        loop {
            if game.state.status != swoq_interface::GameStatus::Active as i32 {
                break;
            }

            let tick_start = Instant::now();

            self.check_level(&game);
            self.update_world(&game.state);

            if let Some(actions) = self.plan_and_execute() {
                tracing::debug!(
                    "GOAP: Executing actions {} for tick {}",
                    actions
                        .iter()
                        .map(|a| format!("{:?}", a))
                        .collect::<Vec<String>>()
                        .join(", "),
                    game.state.tick
                );
                let action_result = self.act_goap(&mut game, actions).await?;

                // Log slow ticks
                let tick_duration = tick_start.elapsed();
                if tick_duration.as_millis() > 100 {
                    tracing::debug!(
                        "⚠️  Tick {} took {:.2}ms result: {:?})",
                        game.state.tick,
                        tick_duration.as_secs_f64() * 1000.0,
                        action_result
                    );
                }

                if action_result != swoq_interface::ActResult::Ok {
                    tracing::debug!("\n❌ Action failed with result: {:?}", action_result);
                    tracing::debug!("🛑 Stopping game due to action error");
                    break;
                }
            } else {
                tracing::debug!(
                    "Skipping tick {} - no executable actions, will replan next iteration",
                    game.state.tick
                );
            }
        }

        let status =
            GameStatus::try_from(game.state.status).unwrap_or(GameStatus::FinishedCanceled);

        // Update statistics
        match status {
            GameStatus::FinishedSuccess => self.successful_runs += 1,
            _ => self.failed_runs += 1,
        }

        self.observer.on_game_finished(
            status,
            game.state.tick,
            self.game_count,
            self.successful_runs,
            self.failed_runs,
        );

        Ok(())
    }

    fn plan_and_execute(&mut self) -> Option<Vec<DirectedAction>> {
        tracing::info!("GOAP: Check replan");
        let (should_replan, is_emergency) = self.executor.needs_replan(&self.world);
        if should_replan {
            if is_emergency {
                tracing::info!("GOAP: EMERGENCY replanning (enemy/health change)");
            } else {
                tracing::info!("GOAP: Scheduled replanning");
            }
            let planner = Planner::new(self.planner_max_depth, self.planner_timeout_ms);
            let plans = planner.plan(&self.world);
            self.executor.set_plans(plans);
            tracing::info!("GOAP: Done replanning");
        }

        // Execute current plans
        let actions = self.executor.step(&mut self.world);

        let goal_names = self.executor.current_goal_names();
        for (player_id, goal_name) in goal_names.iter().enumerate() {
            self.observer
                .on_goal_selected(player_id, goal_name, &self.world);
        }

        actions
    }

    fn check_level(&mut self, game: &crate::infra::swoq::Game) {
        if game.state.level != self.current_level {
            self.observer.on_new_level(game.state.level);
            self.world = WorldState::new(game.map_width, game.map_height, game.visibility_range);
            self.executor = Executor::new();
            self.current_level = game.state.level;
        }
    }

    fn update_world(&mut self, state: &swoq_interface::State) {
        tracing::debug!("\n┌────────────────────────────────────────────────────────────┐");
        tracing::debug!(
            "│ 📊 STATE UPDATE - Tick {}                                  ",
            state.tick
        );
        tracing::debug!("└────────────────────────────────────────────────────────────┘");

        self.world.update(state);
        self.observer.on_state_update(
            state,
            &self.world,
            self.game_count,
            self.successful_runs,
            self.failed_runs,
        );
    }

    async fn act_goap(
        &mut self,
        game: &mut crate::infra::swoq::Game,
        actions: Vec<DirectedAction>,
    ) -> Result<swoq_interface::ActResult, Box<dyn std::error::Error + 'static>> {
        let action1 = actions.first().copied().unwrap_or(DirectedAction::None);
        let action2 = if self.world.players.len() > 1 {
            actions.get(1).copied()
        } else {
            None
        };

        // Log actions for debugging
        for (idx, action) in actions.iter().enumerate() {
            let player = &self.world.players[idx];
            tracing::debug!(
                "Player {}: GOAP Action: {:?} at ({}, {})",
                idx + 1,
                action,
                player.position.x,
                player.position.y
            );
        }

        let action_result = game.act(action1, action2).await?;

        self.observer
            .on_action_result(action1, action2, action_result, &self.world);
        Ok(action_result)
    }
}
