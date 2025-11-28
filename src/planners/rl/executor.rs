//! RL Executor - runs the trained RL agent for inference

use burn::prelude::*;
use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};
use burn::tensor::backend::Backend;

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::action_space::{ActionSpace, MAX_ACTIONS};
use super::actions::{ActionExecutionState, ExecutionStatus, RLActionTrait};
use super::encoder::{EncoderConfig, StateEncoder};
use super::policy::{MAPPOConfig, MAPPOModel};

/// Configuration for RL inference
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Path to model checkpoint
    pub model_path: String,
    /// Whether to use deterministic action selection
    pub deterministic: bool,
    /// Encoder configuration
    pub encoder_config: EncoderConfig,
    /// MAPPO configuration
    pub mappo_config: MAPPOConfig,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            model_path: "checkpoints/final".to_string(),
            deterministic: false,
            encoder_config: EncoderConfig::default(),
            mappo_config: MAPPOConfig::default(),
        }
    }
}

/// State for tracking action execution per player
#[derive(Debug)]
pub struct PlayerExecutionState {
    /// Currently executing action
    pub current_action: Option<Box<dyn RLActionTrait>>,
    /// Execution state for multi-tick actions
    pub execution_state: ActionExecutionState,
    /// Target position for CBS pathfinding
    pub target_position: Option<Position>,
}

impl Default for PlayerExecutionState {
    fn default() -> Self {
        Self {
            current_action: None,
            execution_state: ActionExecutionState::default(),
            target_position: None,
        }
    }
}

/// RL Executor - runs inference with trained model
pub struct RLExecutor<B: Backend> {
    model: MAPPOModel<B>,
    encoder: StateEncoder,
    config: InferenceConfig,
    device: B::Device,
    /// Per-player execution state
    player_states: Vec<PlayerExecutionState>,
}

impl<B: Backend> RLExecutor<B> {
    /// Create a new executor with a trained model
    pub fn new(device: B::Device, config: InferenceConfig) -> Self {
        let model = MAPPOModel::new(&device, &config.encoder_config, &config.mappo_config);
        let encoder = StateEncoder::new(config.encoder_config.clone());

        Self {
            model,
            encoder,
            config,
            device,
            player_states: Vec::new(),
        }
    }

    /// Load model from checkpoint
    pub fn load_model(&mut self, path: &str) {
        let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
        self.model = self
            .model
            .clone()
            .load_file(path, &recorder, &self.device)
            .expect("Failed to load model");
        tracing::info!("Loaded model from {}", path);
    }

    /// Initialize execution state for players
    pub fn init_players(&mut self, num_players: usize) {
        self.player_states = (0..num_players)
            .map(|_| PlayerExecutionState::default())
            .collect();
    }

    /// Select actions for all players
    /// Returns action indices for each player
    pub fn select_actions(&self, world: &WorldState) -> Vec<usize> {
        let num_players = world.players.len();

        // Encode observations
        let local_obs: Vec<Vec<f32>> = (0..num_players)
            .map(|i| self.encoder.encode_local_obs(world, i))
            .collect();

        // Generate action masks
        let action_spaces: Vec<ActionSpace> = (0..num_players)
            .map(|i| ActionSpace::generate(world, i))
            .collect();
        let masks: Vec<Vec<f32>> = action_spaces.iter().map(|s| s.mask_as_floats()).collect();

        // Convert to tensors
        let obs_size = local_obs[0].len();
        let flat_obs: Vec<f32> = local_obs.iter().flatten().copied().collect();
        let flat_masks: Vec<f32> = masks.iter().flatten().copied().collect();

        let obs_tensor: Tensor<B, 3> = Tensor::<B, 1>::from_floats(flat_obs.as_slice(), &self.device)
            .reshape([1, num_players, obs_size]);
        let mask_tensor: Tensor<B, 3> = Tensor::<B, 1>::from_floats(flat_masks.as_slice(), &self.device)
            .reshape([1, num_players, MAX_ACTIONS]);

        // Get actions from policy
        let (actions_tensor, _log_probs) = self.model.get_actions(obs_tensor, mask_tensor);

        // Convert to indices
        let actions: Vec<i64> = actions_tensor.squeeze::<1>(0).into_data().to_vec().unwrap();

        actions.iter().map(|&a| a as usize).collect()
    }

    /// Execute one step for all players
    /// Returns the directed actions for each player
    pub fn step(&mut self, world: &mut WorldState) -> Vec<DirectedAction> {
        let num_players = world.players.len();

        // Ensure we have execution states for all players
        while self.player_states.len() < num_players {
            self.player_states.push(PlayerExecutionState::default());
        }

        // Check if we need new actions for any player
        let mut need_new_actions = false;
        for state in &self.player_states {
            if state.current_action.is_none() {
                need_new_actions = true;
                break;
            }
        }

        // Select new actions if needed
        if need_new_actions {
            let action_indices = self.select_actions(world);

            for (player_idx, &action_idx) in action_indices.iter().enumerate() {
                if self.player_states[player_idx].current_action.is_none() {
                    let action_space = ActionSpace::generate(world, player_idx);
                    if let Some(action) = action_space.clone_action(action_idx) {
                        self.player_states[player_idx].current_action = Some(action);
                        self.player_states[player_idx].execution_state =
                            ActionExecutionState::default();
                    }
                }
            }
        }

        // Execute current actions
        let mut directed_actions = Vec::with_capacity(num_players);

        for player_idx in 0..num_players {
            let state = &mut self.player_states[player_idx];

            let (action, status) = if let Some(ref current_action) = state.current_action {
                current_action.execute(world, player_idx, &mut state.execution_state)
            } else {
                // No valid action, wait
                (DirectedAction::None, ExecutionStatus::Complete)
            };

            directed_actions.push(action);

            // Check if action completed
            match status {
                ExecutionStatus::Complete | ExecutionStatus::Failed => {
                    state.current_action = None;
                    state.execution_state = ActionExecutionState::default();
                    state.target_position = None;
                }
                ExecutionStatus::InProgress | ExecutionStatus::Wait => {
                    // Keep current action
                }
            }
        }

        directed_actions
    }

    /// Get target positions for CBS pathfinding
    pub fn get_target_positions(&mut self, world: &WorldState) -> Vec<Option<Position>> {
        let num_players = world.players.len();
        let mut targets = Vec::with_capacity(num_players);

        for player_idx in 0..num_players {
            if player_idx >= self.player_states.len() {
                targets.push(None);
                continue;
            }

            let state = &mut self.player_states[player_idx];

            // Get target from current action's prepare phase
            if let Some(ref mut action) = state.current_action {
                let target = action.prepare(&mut world.clone(), player_idx);
                state.target_position = target;
                targets.push(target);
            } else {
                targets.push(state.target_position);
            }
        }

        targets
    }

    /// Reset execution state for all players
    pub fn reset(&mut self) {
        for state in &mut self.player_states {
            state.current_action = None;
            state.execution_state = ActionExecutionState::default();
            state.target_position = None;
        }
    }

    /// Get debug info about current actions
    pub fn get_action_names(&self) -> Vec<String> {
        self.player_states
            .iter()
            .map(|s| {
                s.current_action
                    .as_ref()
                    .map(|a| a.name())
                    .unwrap_or_else(|| "None".to_string())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert!(!config.deterministic);
    }
}
