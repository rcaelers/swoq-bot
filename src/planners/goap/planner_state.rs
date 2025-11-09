use crate::infra::Color;
use crate::planners::goap::actions::{ActionExecutionState, GOAPActionTrait};
use crate::state::WorldState;
use std::collections::HashSet;

/// Per-player GOAP planning state
#[derive(Debug, Clone)]
pub struct PlayerPlannerState {
    /// Current action plan sequence for this player (up to max_depth actions)
    pub plan_sequence: Vec<Box<dyn GOAPActionTrait>>,

    /// Index of the current action being executed in the plan sequence
    pub current_action_index: usize,

    /// Execution state for tracking multi-tick actions
    pub execution_state: ActionExecutionState,

    /// When the current action started (in ticks)
    pub action_start_time: Option<u32>,

    /// When the current action is expected to complete (in ticks)
    pub action_end_time: Option<u32>,

    /// Whether the boulder currently in inventory (if any) is unexplored
    pub boulder_is_unexplored: Option<bool>,
}

impl PlayerPlannerState {
    pub fn new() -> Self {
        Self {
            plan_sequence: Vec::new(),
            current_action_index: 0,
            execution_state: ActionExecutionState::default(),
            action_start_time: None,
            action_end_time: None,
            boulder_is_unexplored: None,
        }
    }
}

impl Default for PlayerPlannerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Planning state for GOAP planner
/// Contains WorldState and per-player GOAP-specific state
#[derive(Debug, Clone)]
pub struct PlannerState {
    /// The world state (common between planners)
    pub world: WorldState,

    /// Per-player GOAP planning state
    pub player_states: Vec<PlayerPlannerState>,

    /// Track which plate colors have been touched (for idle activity reward)
    pub plates_touched: HashSet<Color>,
}

impl PlannerState {
    pub fn new(world: WorldState) -> Self {
        let num_players = world.players.len();
        let plates_touched = world.plates_touched.clone();
        tracing::info!("Initializing PlannerState for {} players", num_players);
        Self {
            world,
            player_states: vec![PlayerPlannerState::new(); num_players],
            plates_touched,
        }
    }

    /// Ensure player_states matches the number of players in world
    pub fn sync_player_count(&mut self) {
        while self.player_states.len() < self.world.players.len() {
            self.player_states.push(PlayerPlannerState::new());
        }
        // Sync plates_touched from world
        self.plates_touched = self.world.plates_touched.clone();
    }

    /// Check if replanning is needed
    /// Returns (needs_replan, is_emergency)
    pub fn needs_replan(&self) -> (bool, bool) {
        // Only replan when all plans are complete (empty)
        // ExploreAction will mark itself complete when new objects are discovered
        let plan_complete = self
            .player_states
            .iter()
            .all(|ps| ps.plan_sequence.is_empty());

        tracing::info!("Plan complete: {}", plan_complete);
        // Check for emergency: enemy too close
        let mut is_emergency = false;
        for (player_id, player) in self.world.players.iter().enumerate() {
            tracing::debug!("Checking player {} at position {:?}", player_id, player.position);
            if !player.is_active {
                continue;
            }
            tracing::debug!(
                "Player {} is active with health {}",
                player_id,
                player.health
            );
            let has_sword = player.has_sword;
            let danger_threshold = if has_sword { 2 } else { 3 };

            // Check distance to any enemy
            for enemy_pos in self.world.enemies.get_positions() {
                tracing::debug!(
                    "Checking distance from Player {} at {:?} to Enemy at {:?}",
                    player_id,
                    player.position,
                    enemy_pos
                );
                let dist = self
                    .world
                    .path_distance_to_enemy(player.position, *enemy_pos);
                tracing::debug!(
                    "Distance from Player {} to Enemy at {:?} is {} (threshold: {})",
                    player_id,
                    enemy_pos,
                    dist,
                    danger_threshold
                );
                if dist <= danger_threshold {
                    tracing::warn!(
                        "Emergency replan: Player {} too close to enemy at {:?} (distance: {}, threshold: {})",
                        player_id,
                        enemy_pos,
                        dist,
                        danger_threshold
                    );
                    is_emergency = true;
                    break;
                }
            }

            if is_emergency {
                break;
            }
        }

        (plan_complete || is_emergency, is_emergency)
    }

    /// Clear the plan for a player (e.g., when action completes or fails)
    pub fn clear_plan(&mut self, player_id: usize) {
        self.player_states[player_id].plan_sequence.clear();
        self.player_states[player_id].current_action_index = 0;
        self.player_states[player_id].execution_state = ActionExecutionState::default();
        self.player_states[player_id].action_start_time = None;
        self.player_states[player_id].action_end_time = None;
    }

    /// Get the player whose action will complete first (for time-based planning)
    pub fn get_next_player_to_plan(&self) -> Option<usize> {
        let current_tick = self.world.tick as u32;
        let mut earliest_end_time = u32::MAX;
        let mut next_player = None;

        for (player_id, ps) in self.player_states.iter().enumerate() {
            // Players without plans or with completed actions should be planned first
            if ps.action_end_time.is_none() || ps.action_end_time.unwrap() <= current_tick {
                return Some(player_id);
            }

            // Otherwise, find the player whose action ends earliest
            if let Some(end_time) = ps.action_end_time
                && end_time < earliest_end_time
            {
                earliest_end_time = end_time;
                next_player = Some(player_id);
            }
        }

        next_player
    }
}
