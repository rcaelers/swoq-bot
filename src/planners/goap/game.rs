use std::time::Instant;

use crate::infra::{GameConnection, GameObserver};
use crate::planners::goap::{GOAPExecutor, GOAPPlanner};
use crate::state::WorldState;
use crate::swoq_interface::{self, DirectedAction, GameStatus};

pub struct Game {
    connection: GameConnection,
    observer: Box<dyn GameObserver>,
    world: WorldState,
    current_level: i32,
    planner: GOAPPlanner,
    executor: GOAPExecutor,

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
            planner: GOAPPlanner::new(goap_max_depth, 500),
            executor: GOAPExecutor::new(),
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

        // Reset planner and executor for new game
        self.planner =
            GOAPPlanner::new(self.planner.max_depth, self.planner.timeout.as_millis() as u64);
        self.executor = GOAPExecutor::new();

        loop {
            if game.state.status != swoq_interface::GameStatus::Active as i32 {
                break;
            }

            let tick_start = Instant::now();

            self.check_level(&game);
            self.update_world(&game.state);

            if let Some(actions) = self.plan_and_execute() {
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
        // Initialize or update planner state
        self.planner.update_state(&self.world);

        // Check if we need to replan
        tracing::info!("GOAP: Check replan");
        let (should_replan, is_emergency) = self.planner.needs_replan();
        if should_replan {
            if is_emergency {
                tracing::info!("GOAP: EMERGENCY replanning (enemy/health change)");
            } else {
                tracing::info!("GOAP: Scheduled replanning");
            }
            self.planner.plan(&self.world);
            tracing::info!("GOAP: Done replanning");
        }

        // Update observer with current action goals and paths for each player
        let planner_state = self.planner.state.as_ref().unwrap();
        let mut paths = Vec::new();

        for (player_index, player_state) in planner_state.player_states.iter().enumerate() {
            if !player_state.plan_sequence.is_empty()
                && player_state.current_action_index < player_state.plan_sequence.len()
            {
                let current_action = &player_state.plan_sequence[player_state.current_action_index];
                let action_name = current_action.name();
                self.observer
                    .on_goal_selected(player_index, action_name, &self.world);

                // Get the current path if available
                paths.push(player_state.execution_state.cached_path.clone());
            } else {
                paths.push(None);
            }
        }

        self.observer.on_paths_updated(paths);

        // Execute current plans
        self.executor
            .execute(self.planner.state.as_mut().unwrap(), &mut self.world)
    }

    fn check_level(&mut self, game: &crate::infra::swoq::Game) {
        if game.state.level != self.current_level {
            self.observer.on_new_level(game.state.level);
            // Create a new WorldState for the new level
            self.world = WorldState::new(game.map_width, game.map_height, game.visibility_range);

            // Reset the planner and executor for the new level
            self.planner =
                GOAPPlanner::new(self.planner.max_depth, self.planner.timeout.as_millis() as u64);
            self.executor = GOAPExecutor::new();

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
