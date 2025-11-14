use crate::infra::Color;
use crate::state::WorldState;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct PlayerState {
    /// Whether the boulder currently in inventory (if any) is unexplored
    pub boulder_is_unexplored: Option<bool>,
}

impl PlayerState {
    pub fn new() -> Self {
        Self {
            boulder_is_unexplored: None,
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    /// The world state (common between planners)
    pub world: WorldState,

    /// Per-player GOAP planning state
    pub player_states: Vec<PlayerState>,

    /// Track which plate colors have been touched (for idle activity reward)
    pub plates_touched: HashSet<Color>,
}

impl GameState {
    pub fn new(world: WorldState) -> Self {
        let num_players = world.players.len();
        let plates_touched = world.plates_touched.clone();
        tracing::info!("Initializing PlannerState for {} players", num_players);
        Self {
            world,
            player_states: vec![PlayerState::new(); num_players],
            plates_touched,
        }
    }
}
