use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};

/// Trait for observing game events during execution
pub trait GameObserver {
    /// Called when the game starts
    fn on_game_start(
        &mut self,
        game_id: &str,
        seed: Option<i32>,
        map_width: i32,
        map_height: i32,
        visibility_range: i32,
    );

    /// Called when a new level starts
    fn on_new_level(&mut self, level: i32);

    /// Called when the game state is updated (every tick)
    fn on_state_update(
        &mut self,
        state: &State,
        world: &WorldState,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    );

    /// Called when a goal is selected
    fn on_goal_selected(&mut self, player_index: usize, goal_name: &str, world: &WorldState);

    /// Called to update player paths for visualization
    fn on_paths_updated(&mut self, _paths: Vec<Option<Vec<Position>>>) {
        // Default implementation does nothing
    }

    /// Called when an action is selected
    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState);

    /// Called when an action result is received from the server
    fn on_action_result(
        &mut self,
        action: DirectedAction,
        action2: Option<DirectedAction>,
        result: ActResult,
        world: &WorldState,
    );

    /// Called when the game finishes
    fn on_game_finished(
        &mut self,
        status: GameStatus,
        final_tick: i32,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    );

    /// Called when oscillation is detected
    fn on_oscillation_detected(&mut self, message: &str);
}
