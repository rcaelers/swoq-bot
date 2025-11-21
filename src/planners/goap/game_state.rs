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
pub struct PlayerPlanningState {
    /// Whether the boulder currently in inventory (if any) is unexplored
    pub boulder_is_unexplored: Option<bool>,
}

impl PlayerPlanningState {
    pub fn new() -> Self {
        Self {
            boulder_is_unexplored: None,
        }
    }
}

impl Default for PlayerPlanningState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PlanningState {
    /// Per-player GOAP planning state
    pub player_states: Vec<PlayerPlanningState>,

    /// Track which plate colors have been touched (for idle activity reward)
    pub plates_touched: HashSet<Color>,

    /// Track which resources are claimed by which player (to prevent conflicts)
    pub resource_claims: HashMap<ResourceClaim, usize>,
}

impl PlanningState {
    pub fn new(world: &WorldState) -> Self {
        let num_players = world.players.len();
        let plates_touched = world.plates_touched.clone();
        tracing::info!("Initializing PlanningState for {} players", num_players);
        
        // Initialize player states, checking for boulders in inventory
        let mut player_states = Vec::new();
        for player_idx in 0..num_players {
            let mut state = PlayerPlanningState::new();
            
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
            player_states,
            plates_touched,
            resource_claims: HashMap::new(),
        }
    }
}
