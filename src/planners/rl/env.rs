//! RL Environment - gym-like interface for training

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::action_space::{ActionSpace, MAX_ACTIONS, MultiAgentActionSpace};
use super::actions::{ActionExecutionState, ExecutionStatus, RLActionTrait};
use super::encoder::{EncoderConfig, StateEncoder};

/// Environment configuration
#[derive(Debug, Clone)]
pub struct EnvConfig {
    /// Maximum steps per episode
    pub max_steps: usize,
    /// Reward scaling factor
    pub reward_scale: f32,
    /// Penalty for each step (encourages faster completion)
    pub step_penalty: f32,
    /// Bonus for completing the level
    pub completion_bonus: f32,
    /// Penalty for player death
    pub death_penalty: f32,
    /// Encoder configuration
    pub encoder_config: EncoderConfig,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            max_steps: 500,
            reward_scale: 1.0,
            step_penalty: -0.01,
            completion_bonus: 10.0,
            death_penalty: -5.0,
            encoder_config: EncoderConfig::default(),
        }
    }
}

/// Observation returned by the environment
#[derive(Debug, Clone)]
pub struct Observation {
    /// Per-player local observations [num_players, local_obs_size]
    pub local_obs: Vec<Vec<f32>>,
    /// Global observation for critic [global_obs_size]
    pub global_obs: Vec<f32>,
    /// Action masks [num_players, MAX_ACTIONS]
    pub action_masks: Vec<Vec<f32>>,
}

/// Step result from the environment
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Next observation
    pub observation: Observation,
    /// Shared team reward
    pub reward: f32,
    /// Episode done
    pub done: bool,
    /// Truncated (hit max steps)
    pub truncated: bool,
    /// Additional info
    pub info: StepInfo,
}

/// Additional information from a step
#[derive(Debug, Clone, Default)]
pub struct StepInfo {
    /// Level completed
    pub level_complete: bool,
    /// Any player died
    pub player_died: bool,
    /// Steps taken this episode
    pub steps: usize,
    /// Current level
    pub level: usize,
    /// Team total health
    pub team_health: i32,
    /// Number of enemies remaining
    pub enemies_remaining: usize,
}

/// Multi-agent RL environment
pub struct RLEnv {
    /// Current world state
    world: WorldState,
    /// Initial world state for reset
    initial_world: WorldState,
    /// Environment configuration
    config: EnvConfig,
    /// State encoder
    encoder: StateEncoder,
    /// Current step count
    steps: usize,
    /// Per-player action execution state
    execution_states: Vec<ActionExecutionState>,
    /// Currently executing actions per player
    current_actions: Vec<Option<Box<dyn RLActionTrait>>>,
    /// Previous evaluation score for reward computation
    prev_score: f32,
}

impl RLEnv {
    /// Create a new environment with initial world state
    pub fn new(initial_world: WorldState, config: EnvConfig) -> Self {
        let num_players = initial_world.players.len();
        let encoder = StateEncoder::new(config.encoder_config.clone());
        let prev_score = Self::evaluate_state(&initial_world);

        Self {
            world: initial_world.clone(),
            initial_world,
            config,
            encoder,
            steps: 0,
            execution_states: vec![ActionExecutionState::default(); num_players],
            current_actions: vec![None; num_players],
            prev_score,
        }
    }

    /// Reset the environment to initial state
    pub fn reset(&mut self) -> Observation {
        self.world = self.initial_world.clone();
        self.steps = 0;
        self.execution_states = vec![ActionExecutionState::default(); self.world.players.len()];
        self.current_actions = vec![None; self.world.players.len()];
        self.prev_score = Self::evaluate_state(&self.world);

        self.get_observation()
    }

    /// Reset with a new initial world
    pub fn reset_with_world(&mut self, world: WorldState) -> Observation {
        self.initial_world = world.clone();
        self.world = world;
        self.steps = 0;
        self.execution_states = vec![ActionExecutionState::default(); self.world.players.len()];
        self.current_actions = vec![None; self.world.players.len()];
        self.prev_score = Self::evaluate_state(&self.world);

        self.get_observation()
    }

    /// Get current observation
    pub fn get_observation(&self) -> Observation {
        let action_spaces = MultiAgentActionSpace::generate(&self.world);

        let local_obs: Vec<Vec<f32>> = (0..self.world.players.len())
            .map(|i| self.encoder.encode_local_obs(&self.world, i))
            .collect();

        let global_obs = self.encoder.encode_global_obs(&self.world);
        let action_masks = action_spaces.all_masks();

        Observation {
            local_obs,
            global_obs,
            action_masks,
        }
    }

    /// Take a step with selected action indices for each player
    /// actions: [num_players] action indices into the action space
    pub fn step(&mut self, actions: &[usize]) -> StepResult {
        self.steps += 1;

        // Generate current action spaces
        let mut action_spaces: Vec<ActionSpace> = (0..self.world.players.len())
            .map(|i| ActionSpace::generate(&self.world, i))
            .collect();

        // Get actions and start executing
        let mut low_level_actions: Vec<DirectedAction> = Vec::new();

        for (player_idx, &action_idx) in actions.iter().enumerate() {
            // Get or continue current action
            let action = if self.current_actions[player_idx].is_none() {
                // Select new action from action space
                if let Some(action) = action_spaces[player_idx].clone_action(action_idx) {
                    self.current_actions[player_idx] = Some(action.clone());
                    action
                } else {
                    // Invalid action selected, use wait
                    Box::new(super::actions::WaitAction::new(1)) as Box<dyn RLActionTrait>
                }
            } else {
                self.current_actions[player_idx].as_ref().unwrap().clone()
            };

            // Execute the action
            let (directed_action, status) =
                action.execute(&mut self.world, player_idx, &mut self.execution_states[player_idx]);

            low_level_actions.push(directed_action);

            // Check if action completed
            match status {
                ExecutionStatus::Complete | ExecutionStatus::Failed => {
                    self.current_actions[player_idx] = None;
                    self.execution_states[player_idx] = ActionExecutionState::default();
                }
                ExecutionStatus::InProgress | ExecutionStatus::Wait => {
                    // Keep current action
                }
            }
        }

        // Apply actions to world (this would normally be done by the game server)
        // For training, we simulate the transitions
        // self.world.apply_actions(&low_level_actions);

        // Calculate reward
        let current_score = Self::evaluate_state(&self.world);
        let score_delta = current_score - self.prev_score;
        self.prev_score = current_score;

        let mut reward = score_delta * self.config.reward_scale + self.config.step_penalty;

        // Check for episode termination
        let mut done = false;
        let mut info = StepInfo::default();
        info.steps = self.steps;
        info.level = self.world.level as usize;
        info.team_health = self.world.players.iter().map(|p| p.health).sum();
        info.enemies_remaining = self.world.enemies.get_positions().len();

        // Check if level completed (all live players at exit)
        let level_complete = if let Some(exit_pos) = self.world.exit_position {
            self.world
                .players
                .iter()
                .filter(|p| p.health > 0)
                .all(|p| p.position == exit_pos)
        } else {
            false
        };
        if level_complete {
            done = true;
            info.level_complete = true;
            reward += self.config.completion_bonus;
        }

        // Check if any player died
        if self.world.players.iter().any(|p| p.health <= 0) {
            done = true;
            info.player_died = true;
            reward += self.config.death_penalty;
        }

        // Check for truncation
        let truncated = self.steps >= self.config.max_steps;
        if truncated {
            done = true;
        }

        let observation = self.get_observation();

        StepResult {
            observation,
            reward,
            done,
            truncated,
            info,
        }
    }

    /// Evaluate the current state (from GOAP state_evaluator)
    fn evaluate_state(world: &WorldState) -> f32 {
        let mut score = 0.0;

        // Progress toward exit
        if let Some(exit) = world.exit_position {
            for player in &world.players {
                if player.health > 0 {
                    if let Some(path) = world.find_path(player.position, exit) {
                        // Closer to exit = higher score
                        score -= path.len() as f32 * 0.1;
                    }
                }
            }
        }

        // Health preservation
        for player in &world.players {
            score += player.health as f32 * 2.0;
        }

        // Enemy elimination
        score -= world.enemies.get_positions().len() as f32 * 1.0;

        // Key collection
        for player in &world.players {
            use crate::infra::Color;
            for color in [Color::Red, Color::Green, Color::Blue] {
                if world.has_key(player, color) {
                    score += 3.0;
                }
            }
        }

        // Exploration (more explored = better)
        let explored = world.map.len();
        let total = (world.map.width * world.map.height) as usize;
        score += (explored as f32 / total as f32) * 10.0;

        score
    }

    /// Get the number of players
    pub fn num_players(&self) -> usize {
        self.world.players.len()
    }

    /// Get the local observation size
    pub fn local_obs_size(&self) -> usize {
        self.encoder.local_obs_size()
    }

    /// Get the global observation size
    pub fn global_obs_size(&self) -> usize {
        self.encoder.global_obs_size()
    }

    /// Get current world state (for debugging/visualization)
    pub fn world(&self) -> &WorldState {
        &self.world
    }
}

/// Batch environment for parallel rollout collection
pub struct BatchEnv {
    envs: Vec<RLEnv>,
}

impl BatchEnv {
    pub fn new(envs: Vec<RLEnv>) -> Self {
        Self { envs }
    }

    /// Reset all environments
    pub fn reset_all(&mut self) -> Vec<Observation> {
        self.envs.iter_mut().map(|e| e.reset()).collect()
    }

    /// Step all environments
    pub fn step_all(&mut self, actions: &[Vec<usize>]) -> Vec<StepResult> {
        self.envs
            .iter_mut()
            .zip(actions.iter())
            .map(|(env, acts)| env.step(acts))
            .collect()
    }

    /// Get batch size
    pub fn batch_size(&self) -> usize {
        self.envs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_config_default() {
        let config = EnvConfig::default();
        assert_eq!(config.max_steps, 500);
        assert!((config.completion_bonus - 10.0).abs() < 1e-6);
    }
}
