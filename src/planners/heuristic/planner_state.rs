use crate::planners::heuristic::goals::Goal;
use crate::state::WorldState;

type GoalPair = (Option<Goal>, Option<Goal>);
type GoalPairHistory = [GoalPair; 4];

/// Per-player goal tracking for rule-based planner
#[derive(Debug, Clone)]
pub struct PlayerPlannerState {
    pub current_goal: Option<Goal>,
    pub previous_goal: Option<Goal>,
    /// Oscillation recovery - force random exploration for N ticks
    pub force_random_explore_ticks: i32,
}

impl PlayerPlannerState {
    pub fn new() -> Self {
        Self {
            current_goal: None,
            previous_goal: None,
            force_random_explore_ticks: 0,
        }
    }
}

impl Default for PlayerPlannerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Planning state for rule-based strategy planner
/// Contains WorldState and per-player goal tracking
/// This is only used by the rule-based strategy/goals system, not by GOAP
pub struct PlannerState {
    pub world: WorldState,
    pub player_states: Vec<PlayerPlannerState>,
    // Goal swap detection between players - track last 4 goal pairs (t, t-1, t-2, t-3)
    goal_pair_history: GoalPairHistory,
    goal_pair_index: usize,
}

impl PlannerState {
    pub fn new(world: WorldState) -> Self {
        let num_players = world.players.len();
        Self {
            world,
            player_states: vec![PlayerPlannerState::new(); num_players],
            goal_pair_history: [(None, None), (None, None), (None, None), (None, None)],
            goal_pair_index: 0,
        }
    }

    pub fn sync_player_count(&mut self) {
        while self.player_states.len() < self.world.players.len() {
            self.player_states.push(PlayerPlannerState::new());
        }
    }

    /// Record the current goals for both players (if 2 players exist)
    pub fn record_goal_pair(&mut self, p1_goal: Option<Goal>, p2_goal: Option<Goal>) {
        self.goal_pair_history[self.goal_pair_index] = (p1_goal, p2_goal);
        self.goal_pair_index = (self.goal_pair_index + 1) % 4;
    }

    /// Detect if players are swapping goals
    /// Returns true if the last 4 goal pairs show a swap pattern:
    /// - Goals at t and t-2 are swapped versions of each other
    /// 
    /// Also returns the 4 goal pairs in chronological order for logging
    pub fn is_goal_swapping(&self) -> (bool, GoalPairHistory) {
        // Get goal pairs in chronological order (oldest to newest)
        let latest_idx = if self.goal_pair_index == 0 {
            3
        } else {
            self.goal_pair_index - 1
        };
        
        let t = latest_idx;
        let t_minus_1 = if t == 0 { 3 } else { t - 1 };
        let t_minus_2 = if t_minus_1 == 0 { 3 } else { t_minus_1 - 1 };
        let t_minus_3 = if t_minus_2 == 0 { 3 } else { t_minus_2 - 1 };
        
        let goals_t = &self.goal_pair_history[t];
        let goals_t1 = &self.goal_pair_history[t_minus_1];
        let goals_t2 = &self.goal_pair_history[t_minus_2];
        let goals_t3 = &self.goal_pair_history[t_minus_3];
        
        // Check for swap pattern: goals at t match goals at t-2 swapped, 
        // and goals at t-1 match goals at t-3 swapped
        let is_swapping = 
            // t and t-2 are swaps
            goals_t.0 == goals_t2.1 && goals_t.1 == goals_t2.0 &&
            // t-1 and t-3 are swaps
            goals_t1.0 == goals_t3.1 && goals_t1.1 == goals_t3.0 &&
            // Make sure we're not comparing None == None
            goals_t.0.is_some() && goals_t.1.is_some() &&
            // Make sure players don't have the same goal (no swapping if both have same goal)
            goals_t.0 != goals_t.1;
        
        // Return in chronological order (t-3, t-2, t-1, t)
        (is_swapping, [
            goals_t3.clone(),
            goals_t2.clone(),
            goals_t1.clone(),
            goals_t.clone(),
        ])
    }
}
