use std::time::Instant;

use crate::game_observer::GameObserver;
use crate::goal::Goal;
use crate::strategy::StrategyPlanner;
use crate::swoq::GameConnection;
use crate::swoq_interface::{self, DirectedAction, GameStatus};
use crate::world_state::WorldState;

pub struct Game {
    connection: GameConnection,
    observer: Box<dyn GameObserver>,
    world: WorldState,
    current_level: i32,
    planner: StrategyPlanner,
}

impl Game {
    pub fn new(connection: GameConnection, observer: impl GameObserver + 'static) -> Self {
        Self {
            connection,
            observer: Box::new(observer),
            world: WorldState::new(0, 0, 0),
            current_level: 0,
            planner: StrategyPlanner::new(),
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
        }

        let status =
            GameStatus::try_from(game.state.status).unwrap_or(GameStatus::FinishedCanceled);
        self.observer.on_game_finished(status, game.state.tick);

        Ok(())
    }

    fn check_level(&mut self, game: &crate::swoq::Game) {
        if game.state.level != self.current_level {
            self.observer.on_new_level(game.state.level);
            self.world.reset_for_new_level();
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
        self.observer.on_state_update(state, &self.world);
    }

    fn plan(&mut self) -> Vec<Goal> {
        tracing::debug!("\n┌────────────────────────────────────────────────────────────┐");
        tracing::debug!("│ 🧠 PLANNING PHASE - Selecting goals                        ");
        tracing::debug!("└────────────────────────────────────────────────────────────┘");

        let num_players = self.world.players.len();
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
        if action_result != swoq_interface::ActResult::Ok {
            tracing::debug!("\n❌ Action failed with result: {:?}", action_result);
            tracing::debug!("🛑 Stopping game due to action error");
            return Err(format!("Action failed: {:?}", action_result).into());
        }
        Ok(action_result)
    }
}
