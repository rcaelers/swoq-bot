//! PPO Training loop for MAPPO

use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::*;
use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};
use burn::tensor::backend::AutodiffBackend;

use super::action_space::MAX_ACTIONS;
use super::encoder::EncoderConfig;
use super::env::{EnvConfig, RLEnv};
use super::policy::{MAPPOConfig, MAPPOModel, RolloutBuffer};

/// Training configuration
#[derive(Debug, Clone)]
pub struct TrainConfig {
    /// Number of training iterations
    pub num_iterations: usize,
    /// Steps per rollout
    pub rollout_steps: usize,
    /// Number of parallel environments
    pub num_envs: usize,
    /// Number of PPO epochs per rollout
    pub ppo_epochs: usize,
    /// Mini-batch size for PPO updates
    pub mini_batch_size: usize,
    /// Learning rate
    pub learning_rate: f64,
    /// Max gradient norm for clipping
    pub max_grad_norm: f32,
    /// Path to save checkpoints
    pub checkpoint_dir: String,
    /// Checkpoint save frequency (iterations)
    pub checkpoint_freq: usize,
    /// TensorBoard log directory
    pub log_dir: String,
    /// Whether to use behavioral cloning warmup
    pub use_bc_warmup: bool,
    /// Number of BC warmup iterations
    pub bc_warmup_iterations: usize,
    /// Environment config
    pub env_config: EnvConfig,
    /// Encoder config
    pub encoder_config: EncoderConfig,
    /// MAPPO config
    pub mappo_config: MAPPOConfig,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            num_iterations: 10000,
            rollout_steps: 128,
            num_envs: 8,
            ppo_epochs: 4,
            mini_batch_size: 64,
            learning_rate: 3e-4,
            max_grad_norm: 0.5,
            checkpoint_dir: "checkpoints".to_string(),
            checkpoint_freq: 100,
            log_dir: "logs".to_string(),
            use_bc_warmup: false,
            bc_warmup_iterations: 1000,
            env_config: EnvConfig::default(),
            encoder_config: EncoderConfig::default(),
            mappo_config: MAPPOConfig::default(),
        }
    }
}

/// PPO Trainer - collects rollouts and performs PPO updates
pub struct PPOTrainer<B: AutodiffBackend> {
    model: MAPPOModel<B>,
    config: TrainConfig,
    device: B::Device,
    iteration: usize,
}

impl<B: AutodiffBackend> PPOTrainer<B> {
    pub fn new(device: B::Device, config: TrainConfig) -> Self {
        let model = MAPPOModel::new(&device, &config.encoder_config, &config.mappo_config);

        Self {
            model,
            config,
            device,
            iteration: 0,
        }
    }

    /// Collect a rollout from the environment
    pub fn collect_rollout(&self, env: &mut RLEnv, buffer: &mut RolloutBuffer) {
        buffer.clear();

        let mut obs = env.get_observation();

        for _ in 0..self.config.rollout_steps {
            // Convert observations to tensors
            let local_obs_tensor = self.obs_to_tensor(&obs.local_obs);
            let global_obs_tensor = self.global_obs_to_tensor(&obs.global_obs);
            let mask_tensor = self.masks_to_tensor(&obs.action_masks);

            // Get actions from policy
            let (actions_tensor, log_probs_tensor) = self
                .model
                .get_actions(local_obs_tensor.unsqueeze(), mask_tensor.unsqueeze());

            // Get value estimate
            let value = self.model.get_value(global_obs_tensor.clone().unsqueeze());

            // Convert tensors to vectors
            let actions: Vec<i64> = actions_tensor.squeeze::<1>(0).into_data().to_vec().unwrap();
            let log_probs: Vec<f32> = log_probs_tensor
                .squeeze::<1>(0)
                .into_data()
                .to_vec()
                .unwrap();
            let value_scalar: f32 = value.into_data().to_vec::<f32>().unwrap()[0];

            // Step environment
            let action_indices: Vec<usize> = actions.iter().map(|&a| a as usize).collect();
            let result = env.step(&action_indices);

            // Store transition
            buffer.push(
                obs.local_obs.clone(),
                obs.global_obs.clone(),
                obs.action_masks.clone(),
                actions,
                log_probs,
                result.reward,
                result.done,
                value_scalar,
            );

            obs = result.observation;

            if result.done {
                obs = env.reset();
            }
        }
    }

    /// Perform PPO update on collected rollout
    pub fn ppo_update(&mut self, buffer: &RolloutBuffer) -> (f32, f32, f32) {
        // Get last value for GAE computation
        let last_global_obs = buffer.global_obs.last().unwrap();
        let last_value = if *buffer.dones.last().unwrap_or(&true) {
            0.0
        } else {
            let global_tensor = self.global_obs_to_tensor(last_global_obs).unsqueeze();
            let val = self.model.get_value(global_tensor);
            val.into_data().to_vec::<f32>().unwrap()[0]
        };

        // Compute returns and advantages
        let (returns, advantages) = buffer.compute_returns_and_advantages(
            last_value,
            self.config.mappo_config.gamma,
            self.config.mappo_config.gae_lambda,
        );

        // Normalize advantages
        let adv_mean: f32 = advantages.iter().sum::<f32>() / advantages.len() as f32;
        let adv_var: f32 = advantages
            .iter()
            .map(|a| (a - adv_mean).powi(2))
            .sum::<f32>()
            / advantages.len() as f32;
        let adv_std = adv_var.sqrt().max(1e-8);
        let normalized_advantages: Vec<f32> = advantages
            .iter()
            .map(|a| (a - adv_mean) / adv_std)
            .collect();

        // Create optimizer
        let optim_config = AdamConfig::new();
        let mut optimizer = optim_config.init::<B, MAPPOModel<B>>();

        let mut total_policy_loss = 0.0f32;
        let mut total_value_loss = 0.0f32;
        let mut total_entropy = 0.0f32;
        let mut num_updates = 0usize;

        for _ in 0..self.config.ppo_epochs {
            // Create mini-batches
            let indices: Vec<usize> = (0..buffer.len()).collect();

            for batch_start in (0..buffer.len()).step_by(self.config.mini_batch_size) {
                let batch_end = (batch_start + self.config.mini_batch_size).min(buffer.len());
                let batch_indices: Vec<usize> = indices[batch_start..batch_end].to_vec();

                if batch_indices.is_empty() {
                    continue;
                }

                // Gather batch data
                let batch_local_obs: Vec<Vec<Vec<f32>>> = batch_indices
                    .iter()
                    .map(|&i| buffer.local_obs[i].clone())
                    .collect();
                let batch_global_obs: Vec<Vec<f32>> = batch_indices
                    .iter()
                    .map(|&i| buffer.global_obs[i].clone())
                    .collect();
                let batch_masks: Vec<Vec<Vec<f32>>> = batch_indices
                    .iter()
                    .map(|&i| buffer.masks[i].clone())
                    .collect();
                let batch_actions: Vec<Vec<i64>> = batch_indices
                    .iter()
                    .map(|&i| buffer.actions[i].clone())
                    .collect();
                let batch_old_log_probs: Vec<Vec<f32>> = batch_indices
                    .iter()
                    .map(|&i| buffer.log_probs[i].clone())
                    .collect();
                let batch_returns: Vec<f32> = batch_indices.iter().map(|&i| returns[i]).collect();
                let batch_advantages: Vec<f32> = batch_indices
                    .iter()
                    .map(|&i| normalized_advantages[i])
                    .collect();

                // Convert to tensors
                let local_obs_tensor = self.batch_obs_to_tensor(&batch_local_obs);
                let global_obs_tensor = self.batch_global_obs_to_tensor(&batch_global_obs);
                let masks_tensor = self.batch_masks_to_tensor(&batch_masks);
                let actions_tensor = self.batch_actions_to_tensor(&batch_actions);
                let old_log_probs_tensor = self.batch_log_probs_to_tensor(&batch_old_log_probs);
                let returns_tensor =
                    Tensor::<B, 1>::from_floats(batch_returns.as_slice(), &self.device);
                let advantages_tensor =
                    Tensor::<B, 1>::from_floats(batch_advantages.as_slice(), &self.device);

                // Evaluate actions with current policy
                let (new_log_probs, entropy, values) = self.model.evaluate_actions(
                    local_obs_tensor,
                    global_obs_tensor,
                    masks_tensor,
                    actions_tensor,
                );

                // Sum log probs across agents for joint action probability
                let new_log_probs_sum = new_log_probs.sum_dim(1).squeeze(1);
                let old_log_probs_sum = old_log_probs_tensor.sum_dim(1).squeeze(1);

                // PPO clipped objective
                let ratio = (new_log_probs_sum.clone() - old_log_probs_sum).exp();
                let clip_epsilon = self.config.mappo_config.clip_epsilon;
                let clipped_ratio = ratio.clone().clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon);

                let surr1 = ratio * advantages_tensor.clone();
                let surr2 = clipped_ratio * advantages_tensor;
                let policy_loss = -surr1.min_pair(surr2).mean();

                // Value loss
                let value_loss = (values - returns_tensor).powf_scalar(2.0).mean();

                // Entropy bonus
                let entropy_mean = entropy.mean();

                // Total loss
                let loss = policy_loss.clone()
                    + value_loss.clone() * self.config.mappo_config.value_coef
                    - entropy_mean.clone() * self.config.mappo_config.entropy_coef;

                // Backward pass
                let grads = loss.backward();
                let grads = GradientsParams::from_grads(grads, &self.model);

                // Update model
                self.model = optimizer.step(self.config.learning_rate, self.model.clone(), grads);

                // Track losses
                total_policy_loss += policy_loss.into_data().to_vec::<f32>().unwrap()[0];
                total_value_loss += value_loss.into_data().to_vec::<f32>().unwrap()[0];
                total_entropy += entropy_mean.into_data().to_vec::<f32>().unwrap()[0];
                num_updates += 1;
            }
        }

        let avg_policy_loss = if num_updates > 0 {
            total_policy_loss / num_updates as f32
        } else {
            0.0
        };
        let avg_value_loss = if num_updates > 0 {
            total_value_loss / num_updates as f32
        } else {
            0.0
        };
        let avg_entropy = if num_updates > 0 {
            total_entropy / num_updates as f32
        } else {
            0.0
        };

        (avg_policy_loss, avg_value_loss, avg_entropy)
    }

    /// Run the training loop
    pub fn train(&mut self, mut env: RLEnv) {
        let mut buffer = RolloutBuffer::new();

        tracing::info!(
            "Starting training for {} iterations",
            self.config.num_iterations
        );

        // BC warmup if enabled
        if self.config.use_bc_warmup {
            tracing::info!(
                "Running BC warmup for {} iterations",
                self.config.bc_warmup_iterations
            );
            // BC warmup would be implemented here using the bc module
        }

        for iteration in 0..self.config.num_iterations {
            self.iteration = iteration;

            // Collect rollout
            self.collect_rollout(&mut env, &mut buffer);

            // PPO update
            let (policy_loss, value_loss, entropy) = self.ppo_update(&buffer);

            // Logging
            if iteration % 10 == 0 {
                tracing::info!(
                    "Iteration {}: policy_loss={:.4}, value_loss={:.4}, entropy={:.4}",
                    iteration,
                    policy_loss,
                    value_loss,
                    entropy
                );
            }

            // Save checkpoint
            if iteration % self.config.checkpoint_freq == 0 {
                self.save_checkpoint(&format!(
                    "{}/checkpoint_{}",
                    self.config.checkpoint_dir, iteration
                ));
            }
        }

        // Final checkpoint
        self.save_checkpoint(&format!("{}/final", self.config.checkpoint_dir));
        tracing::info!("Training complete!");
    }

    /// Save model checkpoint
    pub fn save_checkpoint(&self, path: &str) {
        let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
        self.model
            .clone()
            .save_file(path, &recorder)
            .expect("Failed to save checkpoint");
        tracing::info!("Saved checkpoint to {}", path);
    }

    /// Load model checkpoint
    pub fn load_checkpoint(&mut self, path: &str) {
        let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
        self.model = self
            .model
            .clone()
            .load_file(path, &recorder, &self.device)
            .expect("Failed to load checkpoint");
        tracing::info!("Loaded checkpoint from {}", path);
    }

    // Helper functions for tensor conversion

    fn obs_to_tensor(&self, local_obs: &[Vec<f32>]) -> Tensor<B, 2> {
        let num_agents = local_obs.len();
        let obs_size = local_obs[0].len();
        let flat: Vec<f32> = local_obs.iter().flatten().copied().collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device).reshape([num_agents, obs_size])
    }

    fn global_obs_to_tensor(&self, global_obs: &[f32]) -> Tensor<B, 1> {
        Tensor::<B, 1>::from_floats(global_obs, &self.device)
    }

    fn masks_to_tensor(&self, masks: &[Vec<f32>]) -> Tensor<B, 2> {
        let num_agents = masks.len();
        let flat: Vec<f32> = masks.iter().flatten().copied().collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device).reshape([num_agents, MAX_ACTIONS])
    }

    fn batch_obs_to_tensor(&self, batch: &[Vec<Vec<f32>>]) -> Tensor<B, 3> {
        let batch_size = batch.len();
        let num_agents = batch[0].len();
        let obs_size = batch[0][0].len();
        let flat: Vec<f32> = batch
            .iter()
            .flat_map(|b| b.iter().flat_map(|a| a.iter().copied()))
            .collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device)
            .reshape([batch_size, num_agents, obs_size])
    }

    fn batch_global_obs_to_tensor(&self, batch: &[Vec<f32>]) -> Tensor<B, 2> {
        let batch_size = batch.len();
        let obs_size = batch[0].len();
        let flat: Vec<f32> = batch.iter().flatten().copied().collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device).reshape([batch_size, obs_size])
    }

    fn batch_masks_to_tensor(&self, batch: &[Vec<Vec<f32>>]) -> Tensor<B, 3> {
        let batch_size = batch.len();
        let num_agents = batch[0].len();
        let flat: Vec<f32> = batch
            .iter()
            .flat_map(|b| b.iter().flat_map(|a| a.iter().copied()))
            .collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device).reshape([
            batch_size,
            num_agents,
            MAX_ACTIONS,
        ])
    }

    fn batch_actions_to_tensor(&self, batch: &[Vec<i64>]) -> Tensor<B, 2, Int> {
        let batch_size = batch.len();
        let num_agents = batch[0].len();
        let flat: Vec<i64> = batch.iter().flatten().copied().collect();
        Tensor::<B, 1, Int>::from_ints(flat.as_slice(), &self.device)
            .reshape([batch_size, num_agents])
    }

    fn batch_log_probs_to_tensor(&self, batch: &[Vec<f32>]) -> Tensor<B, 2> {
        let batch_size = batch.len();
        let num_agents = batch[0].len();
        let flat: Vec<f32> = batch.iter().flatten().copied().collect();
        Tensor::<B, 1>::from_floats(flat.as_slice(), &self.device).reshape([batch_size, num_agents])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_train_config_default() {
        let config = TrainConfig::default();
        assert_eq!(config.num_iterations, 10000);
        assert_eq!(config.rollout_steps, 128);
        assert_eq!(config.num_envs, 8);
    }
}
