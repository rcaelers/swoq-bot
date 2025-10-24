use crate::swoq_interface::{DirectedAction, GameStatus, State};
use crate::world_state::WorldState;

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
    fn on_new_level(&mut self, level: i32, previous_level: i32);

    /// Called when the game state is updated (every tick)
    fn on_state_update(&mut self, state: &State, world: &WorldState);

    /// Called when an action is selected
    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState);

    /// Called when the game finishes
    fn on_game_finished(&mut self, status: GameStatus, final_tick: i32);
}
