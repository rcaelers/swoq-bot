use std::time::Instant;

use crate::game_observer::GameObserver;
use crate::goals::Goal;
use crate::strategies::StrategyPlanner;
use crate::swoq::GameConnection;
use crate::swoq_interface::{self, DirectedAction, GameStatus};
use crate::types::Position;
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
            let p1_goal = goals.first().cloned();
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

        // Post-execution safety check: Prevent door crushing in 2-player mode
        if num_players == 2 {
            Self::check_door_crush_safety(&self.world, &mut results);
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

    /// Post-execution safety check: If one player is on a plate and another is near a door,
    /// only force evacuation if the player near the door is actually moving toward it
    fn check_door_crush_safety(world: &WorldState, results: &mut [(Goal, DirectedAction)]) {
        use crate::types::Color;

        // Check each player to see if they're on a pressure plate
        for player_idx in 0..2 {
            let other_idx = 1 - player_idx;
            let player_pos = world.players[player_idx].position;

            // Check if this player is currently on a pressure plate
            let on_plate_color: Option<Color> = [Color::Red, Color::Green, Color::Blue]
                .iter()
                .find_map(|&color| {
                    if let Some(plates) = world.pressure_plates.get_positions(color)
                        && plates.contains(&player_pos)
                    {
                        return Some(color);
                    }
                    None
                });

            if let Some(color) = on_plate_color {
                let other_player_pos = world.players[other_idx].position;
                // Clone the action to avoid borrow checker issues when mutating results
                let other_action = results[other_idx].1;

                // Check if the other player is at/near a door of the same color
                if let Some(doors) = world.doors.get_positions(color) {
                    for &door_pos in doors {
                        let other_on_door = other_player_pos == door_pos;

                        if other_on_door {
                            // Check if player on plate has a goal that would take them off the plate
                            let (player_goal, _) = &results[player_idx];
                            let player_leaving_plate = !matches!(
                                player_goal,
                                Goal::WaitOnTile(c, pos) if c == &color && pos == &player_pos
                            );

                            // If other player is ON the door, this player CANNOT leave the plate
                            if player_leaving_plate {
                                tracing::debug!(
                                    "Post-execution: P{} is ON {:?} door {:?}, P{} MUST stay on {:?} plate at {:?}",
                                    other_idx + 1,
                                    color,
                                    door_pos,
                                    player_idx + 1,
                                    color,
                                    player_pos
                                );
                                // Override: force stay on plate with no action
                                results[player_idx] =
                                    (Goal::WaitOnTile(color, player_pos), DirectedAction::None);
                            }
                        }

                        // Check if other player is adjacent and moving TOWARD the door
                        let other_near_door = other_player_pos.is_adjacent(&door_pos);
                        if other_near_door {
                            // Determine if the action moves toward the door
                            let next_pos = Self::get_next_position(other_player_pos, other_action);
                            let moving_toward_door = next_pos == door_pos;

                            // Check if player on plate is leaving
                            let (player_goal, _) = &results[player_idx];
                            let player_leaving_plate = !matches!(
                                player_goal,
                                Goal::WaitOnTile(c, pos) if c == &color && pos == &player_pos
                            );

                            // Only force wait if other player is moving toward the door
                            if player_leaving_plate && moving_toward_door {
                                tracing::debug!(
                                    "Post-execution: P{} on {:?} plate leaving, P{} moving toward {:?} door {:?} - forcing WaitOnTile",
                                    player_idx + 1,
                                    color,
                                    other_idx + 1,
                                    color,
                                    door_pos
                                );
                                results[other_idx] = (
                                    Goal::WaitOnTile(color, other_player_pos),
                                    DirectedAction::None,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Helper to determine next position based on an action
    fn get_next_position(current: Position, action: DirectedAction) -> Position {
        match action {
            DirectedAction::MoveNorth => Position::new(current.x, current.y - 1),
            DirectedAction::MoveSouth => Position::new(current.x, current.y + 1),
            DirectedAction::MoveEast => Position::new(current.x + 1, current.y),
            DirectedAction::MoveWest => Position::new(current.x - 1, current.y),
            _ => current,
        }
    }
}
