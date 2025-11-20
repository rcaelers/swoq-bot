use crate::infra::{Color, Position};
use crate::state::WorldState;
use std::collections::{HashSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceClaim {
    Key(Color),
    Door(Color),
    Sword(Position),
    PressurePlate(Color),
    Health(Position),
    // Can add more claim types as needed
}

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

    /// Track which resources are claimed by which player (to prevent conflicts)
    pub resource_claims: HashMap<ResourceClaim, usize>,
}

impl GameState {
    pub fn new(world: WorldState) -> Self {
        let num_players = world.players.len();
        let plates_touched = world.plates_touched.clone();
        tracing::info!("Initializing PlannerState for {} players", num_players);
        
        // Initialize player states, checking for boulders in inventory
        let mut player_states = Vec::new();
        for player_idx in 0..num_players {
            let mut state = PlayerState::new();
            
            // If player has a boulder in inventory, mark it as unexplored
            // This ensures DropBoulder actions are generated when replanning
            if world.players[player_idx].inventory == crate::swoq_interface::Inventory::Boulder {
                tracing::info!(
                    "Player {} has boulder in inventory during initialization, marking as unexplored",
                    player_idx
                );
                state.boulder_is_unexplored = Some(true);
            }
            
            player_states.push(state);
        }
        
        Self {
            world,
            player_states,
            plates_touched,
            resource_claims: HashMap::new(),
        }
    }
}
