use std::time::Instant;

use crate::game_observer::GameObserver;
use crate::goals::Goal;
use crate::strategies::StrategyPlanner;
use crate::swoq::GameConnection;
use crate::swoq_interface::{self, DirectedAction, GameStatus};
use crate::world_state::WorldState;

pub struct Game {
    connection: GameConnection,
    observer: Box<dyn GameObserver>,
    world: WorldState,
    current_level: i32,
    planner: StrategyPlanner,
    
    // Game statistics (persistent across levels)
    pub successful_runs: i32,
    pub failed_runs: i32,
    pub game_count: i32,
}

impl Game {
    pub fn new(connection: GameConnection, observer: impl GameObserver + 'static) -> Self {
        Self {
            connection,
            observer: Box::new(observer),
            world: WorldState::new(0, 0, 0),
            current_level: 0,
            planner: StrategyPlanner::new(),
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

        self.world = WorldState::new(game.map_width, game.map_height, game.visibility_range);
        self.current_level = game.state.level;

        loop {
            if game.state.status != swoq_interface::GameStatus::Active as i32 {
                break;
            }

            let tick_start = Instant::now();

            self.check_level(&game);
            self.update_world(&game.state);
            let goals = self.plan();
            let actions = self.excute(goals);
            let action_result = self.act(&mut game, actions).await?;

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

    fn check_level(&mut self, game: &crate::swoq::Game) {
        if game.state.level != self.current_level {
            self.observer.on_new_level(game.state.level);
            // Create a new WorldState for the new level
            self.world = WorldState::new(game.map_width, game.map_height, game.visibility_range);
            self.planner = StrategyPlanner::new();
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

    fn plan(&mut self) -> Vec<Goal> {
        tracing::debug!("\n┌────────────────────────────────────────────────────────────┐");
        tracing::debug!("│ 🧠 PLANNING PHASE - Selecting goals                        ");
        tracing::debug!("└────────────────────────────────────────────────────────────┘");

        let num_players = self.world.players.len();

        // Decrement forced random exploration counter
        for player_index in 0..num_players {
            if self.world.players[player_index].force_random_explore_ticks > 0 {
                self.world.players[player_index].force_random_explore_ticks -= 1;
            }
        }

        for player_index in 0..num_players {
            if let Some(dest) = self.world.players[player_index].current_destination
                && self.world.players[player_index].position == dest
            {
                // Reached destination - clear it to select a new goal
                self.world.players[player_index].current_destination = None;
            }
        }

        for player_index in 0..num_players {
            self.world.players[player_index].current_goal = None;
        }

        let goals = self.planner.select_goal(&self.world);

        // Display selected goals
        for (player_index, goal) in goals.iter().enumerate() {
            if player_index < num_players {
                let frontier_size = self.world.players[player_index].unexplored_frontier.len();
                let player_pos = self.world.players[player_index].position;
                let player_tile = self.world.map.get(&player_pos);
                let current_dest = self.world.players[player_index].current_destination;

                tracing::debug!(
                    "  Player {}: {:?}, frontier size: {}, tile: {:?}, dest: {:?}",
                    player_index + 1,
                    goal,
                    frontier_size,
                    player_tile,
                    current_dest
                );
            }
        }

        goals
    }

    fn excute(&mut self, goals: Vec<Goal>) -> Vec<(Goal, DirectedAction)> {
        let num_players = self.world.players.len();

        tracing::debug!("\n┌────────────────────────────────────────────────────────────┐");
        tracing::debug!("│ ⚡ EXECUTING ACTIONS - Planning actions for goals           ");
        tracing::debug!("└────────────────────────────────────────────────────────────┘");

        // Check for goal swapping between players (only if 2 players)
        if num_players == 2 {
            let p1_goal = goals.get(0).cloned();
            let p2_goal = goals.get(1).cloned();

            self.world
                .record_goal_pair(p1_goal.clone(), p2_goal.clone());

            let (is_swapping, history) = self.world.is_goal_swapping();
            if is_swapping {
                // Log to UI
                let log_message = format!(
                    "⚠️  Players SWAPPING goals!\n   t-3: P1={:?}, P2={:?}\n   t-2: P1={:?}, P2={:?}\n   t-1: P1={:?}, P2={:?}\n   t:   P1={:?}, P2={:?}",
                    history[0].0,
                    history[0].1,
                    history[1].0,
                    history[1].1,
                    history[2].0,
                    history[2].1,
                    history[3].0,
                    history[3].1
                );
                tracing::warn!("{}", log_message);
                self.observer.on_oscillation_detected(&log_message);

                // Force player 1 to random exploration for 10 ticks
                self.world.players[0].force_random_explore_ticks = 10;
            }
        }

        let mut results = Vec::new();
        for (player_index, goal) in goals.into_iter().enumerate() {
            if player_index < num_players {
                self.world.players[player_index].current_goal = Some(goal.clone());
                let action = goal
                    .execute_for_player(&mut self.world, player_index)
                    .unwrap_or(DirectedAction::None);

                self.world.players[player_index].previous_goal = Some(goal.clone());
                results.push((goal, action));
            }
        }

        for (idx, (goal, action)) in results.iter().enumerate() {
            let player = &self.world.players[idx];
            tracing::debug!("Player {}: Selected Goal: {:?}", idx + 1, goal);
            tracing::debug!(
                "Player {}: Action selected: {:?} at ({}, {})",
                idx + 1,
                action,
                player.position.x,
                player.position.y
            );

            self.observer.on_goal_selected(goal, &self.world);
            if idx == 0 {
                self.observer.on_action_selected(*action, &self.world);
            }
        }

        results
    }

    async fn act(
        &mut self,
        game: &mut crate::swoq::Game,
        actions: Vec<(Goal, DirectedAction)>,
    ) -> Result<swoq_interface::ActResult, Box<dyn std::error::Error + 'static>> {
        let player_actions: &[(Goal, DirectedAction)] = &actions;
        let action1 = player_actions
            .first()
            .map(|(_, a)| *a)
            .unwrap_or(DirectedAction::None);
        let action2 = if self.world.players.len() > 1 {
            player_actions.get(1).map(|(_, a)| Some(*a)).unwrap_or(None)
        } else {
            None
        };

        let action_result = game.act(action1, action2).await?;

        self.observer
            .on_action_result(action1, action2, action_result, &self.world);
        Ok(action_result)
    }
}
