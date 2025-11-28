//! MAPPO Policy and Critic networks using Burn framework

use burn::module::Module;
use burn::nn::{Linear, LinearConfig, Relu};
use burn::prelude::*;
use burn::tensor::activation::softmax;

use super::action_space::MAX_ACTIONS;
use super::encoder::{EncoderConfig, StateEncoder};

/// Configuration for MAPPO networks
#[derive(Debug, Config)]
pub struct MAPPOConfig {
    /// Hidden layer size for actor network
    pub actor_hidden_size: usize,
    /// Number of hidden layers for actor
    pub actor_num_layers: usize,
    /// Hidden layer size for critic network
    pub critic_hidden_size: usize,
    /// Number of hidden layers for critic
    pub critic_num_layers: usize,
    /// Entropy coefficient for exploration
    pub entropy_coef: f32,
    /// Value loss coefficient
    pub value_coef: f32,
    /// PPO clip parameter
    pub clip_epsilon: f32,
    /// Discount factor
    pub gamma: f32,
    /// GAE lambda
    pub gae_lambda: f32,
}

impl Default for MAPPOConfig {
    fn default() -> Self {
        Self {
            actor_hidden_size: 256,
            actor_num_layers: 2,
            critic_hidden_size: 512,
            critic_num_layers: 3,
            entropy_coef: 0.01,
            value_coef: 0.5,
            clip_epsilon: 0.2,
            gamma: 0.99,
            gae_lambda: 0.95,
        }
    }
}

/// Actor network - outputs action probabilities for a single agent
/// Uses local observation only (decentralized execution)
#[derive(Module, Debug)]
pub struct Actor<B: Backend> {
    /// Input layer
    input: Linear<B>,
    /// Hidden layers
    hidden: Vec<Linear<B>>,
    /// Output layer (logits for each action)
    output: Linear<B>,
    /// Activation function
    activation: Relu,
}

impl<B: Backend> Actor<B> {
    pub fn new(device: &B::Device, local_obs_size: usize, config: &MAPPOConfig) -> Self {
        let input = LinearConfig::new(local_obs_size, config.actor_hidden_size).init(device);

        let mut hidden = Vec::new();
        for _ in 0..config.actor_num_layers - 1 {
            hidden.push(
                LinearConfig::new(config.actor_hidden_size, config.actor_hidden_size).init(device),
            );
        }

        let output = LinearConfig::new(config.actor_hidden_size, MAX_ACTIONS).init(device);

        Self {
            input,
            hidden,
            output,
            activation: Relu::new(),
        }
    }

    /// Forward pass returning raw logits
    pub fn forward(&self, obs: Tensor<B, 2>) -> Tensor<B, 2> {
        let mut x = self.activation.forward(self.input.forward(obs));

        for layer in &self.hidden {
            x = self.activation.forward(layer.forward(x));
        }

        self.output.forward(x)
    }

    /// Get action probabilities with masking
    /// mask: [batch_size, MAX_ACTIONS] where 1.0 = valid, 0.0 = invalid
    pub fn get_probs(&self, obs: Tensor<B, 2>, mask: Tensor<B, 2>) -> Tensor<B, 2> {
        let logits = self.forward(obs);

        // Apply mask: set invalid actions to -inf before softmax
        let neg_inf = Tensor::full(logits.shape(), f32::NEG_INFINITY, &logits.device());
        let masked_logits = mask.clone() * logits + (Tensor::ones_like(&mask) - mask) * neg_inf;

        softmax(masked_logits, 1)
    }

    /// Sample an action from the policy
    /// Returns (action_index, log_prob)
    pub fn sample_action(
        &self,
        obs: Tensor<B, 2>,
        mask: Tensor<B, 2>,
    ) -> (Tensor<B, 1, Int>, Tensor<B, 1>) {
        let probs = self.get_probs(obs, mask);

        // Sample from categorical distribution
        // Using argmax of (log(probs) - log(-log(uniform))) for Gumbel-max trick
        let uniform = Tensor::<B, 2>::random(
            probs.shape(),
            burn::tensor::Distribution::Uniform(0.0, 1.0),
            &probs.device(),
        );
        let gumbel = -(-uniform.log()).log();
        let noisy_logits = probs.clone().log() + gumbel;

        let action: Tensor<B, 2, Int> = noisy_logits.argmax(1);

        // Get log probability of sampled action
        let log_probs = probs.log();

        // Gather log prob at action index
        let action_log_prob = Self::gather_1d(log_probs, action.clone());

        // Squeeze action to 1D
        (action.squeeze(1), action_log_prob)
    }

    /// Evaluate actions - get log probs and entropy for given actions
    pub fn evaluate_actions(
        &self,
        obs: Tensor<B, 2>,
        mask: Tensor<B, 2>,
        actions: Tensor<B, 1, Int>,
    ) -> (Tensor<B, 1>, Tensor<B, 1>) {
        let probs = self.get_probs(obs, mask);
        let log_probs = probs.clone().log();

        // Reshape actions to 2D for gather
        let batch_size = actions.dims()[0];
        let actions_2d = actions.reshape([batch_size, 1]);

        // Gather log prob at action index
        let action_log_prob = Self::gather_1d(log_probs.clone(), actions_2d);

        // Calculate entropy: -sum(p * log(p))
        let entropy = -(probs.clone() * log_probs).sum_dim(1).squeeze(1);

        (action_log_prob, entropy)
    }

    /// Helper to gather values at indices (1D gather along dim 1)
    fn gather_1d(tensor: Tensor<B, 2>, indices: Tensor<B, 2, Int>) -> Tensor<B, 1> {
        tensor.gather(1, indices).squeeze(1)
    }
}

/// Centralized Critic network - uses global observation
#[derive(Module, Debug)]
pub struct Critic<B: Backend> {
    /// Input layer
    input: Linear<B>,
    /// Hidden layers
    hidden: Vec<Linear<B>>,
    /// Output layer (single value)
    output: Linear<B>,
    /// Activation function
    activation: Relu,
}

impl<B: Backend> Critic<B> {
    pub fn new(device: &B::Device, global_obs_size: usize, config: &MAPPOConfig) -> Self {
        let input = LinearConfig::new(global_obs_size, config.critic_hidden_size).init(device);

        let mut hidden = Vec::new();
        for _ in 0..config.critic_num_layers - 1 {
            hidden.push(
                LinearConfig::new(config.critic_hidden_size, config.critic_hidden_size)
                    .init(device),
            );
        }

        let output = LinearConfig::new(config.critic_hidden_size, 1).init(device);

        Self {
            input,
            hidden,
            output,
            activation: Relu::new(),
        }
    }

    /// Forward pass returning state value
    pub fn forward(&self, global_obs: Tensor<B, 2>) -> Tensor<B, 1> {
        let mut x = self.activation.forward(self.input.forward(global_obs));

        for layer in &self.hidden {
            x = self.activation.forward(layer.forward(x));
        }

        self.output.forward(x).squeeze(1)
    }
}

/// Complete MAPPO model with shared critic and per-agent actors
#[derive(Module, Debug)]
pub struct MAPPOModel<B: Backend> {
    /// Per-agent actor networks (shared weights for homogeneous agents)
    pub actor: Actor<B>,
    /// Centralized critic
    pub critic: Critic<B>,
}

impl<B: Backend> MAPPOModel<B> {
    pub fn new(
        device: &B::Device,
        encoder_config: &EncoderConfig,
        mappo_config: &MAPPOConfig,
    ) -> Self {
        let encoder = StateEncoder::new(encoder_config.clone());
        let local_obs_size = encoder.local_obs_size();
        let global_obs_size = encoder.global_obs_size();

        Self {
            actor: Actor::new(device, local_obs_size, mappo_config),
            critic: Critic::new(device, global_obs_size, mappo_config),
        }
    }

    /// Get actions for all agents
    /// local_obs: [batch_size, num_agents, local_obs_size]
    /// masks: [batch_size, num_agents, MAX_ACTIONS]
    /// Returns: actions [batch_size, num_agents], log_probs [batch_size, num_agents]
    pub fn get_actions(
        &self,
        local_obs: Tensor<B, 3>,
        masks: Tensor<B, 3>,
    ) -> (Tensor<B, 2, Int>, Tensor<B, 2>) {
        let [batch_size, num_agents, obs_size] = local_obs.dims();

        // Reshape to process all agents at once
        let flat_obs = local_obs.reshape([batch_size * num_agents, obs_size]);
        let flat_masks = masks.reshape([batch_size * num_agents, MAX_ACTIONS]);

        let (flat_actions, flat_log_probs) = self.actor.sample_action(flat_obs, flat_masks);

        // Reshape back
        let actions = flat_actions.reshape([batch_size, num_agents]);
        let log_probs = flat_log_probs.reshape([batch_size, num_agents]);

        (actions, log_probs)
    }

    /// Get value estimate from centralized critic
    pub fn get_value(&self, global_obs: Tensor<B, 2>) -> Tensor<B, 1> {
        self.critic.forward(global_obs)
    }

    /// Evaluate actions for PPO loss computation
    pub fn evaluate_actions(
        &self,
        local_obs: Tensor<B, 3>,
        global_obs: Tensor<B, 2>,
        masks: Tensor<B, 3>,
        actions: Tensor<B, 2, Int>,
    ) -> (Tensor<B, 2>, Tensor<B, 2>, Tensor<B, 1>) {
        let [batch_size, num_agents, obs_size] = local_obs.dims();

        // Reshape for actor
        let flat_obs = local_obs.reshape([batch_size * num_agents, obs_size]);
        let flat_masks = masks.reshape([batch_size * num_agents, MAX_ACTIONS]);
        let flat_actions = actions.clone().reshape([batch_size * num_agents]);

        let (flat_log_probs, flat_entropy) =
            self.actor
                .evaluate_actions(flat_obs, flat_masks, flat_actions);

        // Reshape back
        let log_probs = flat_log_probs.reshape([batch_size, num_agents]);
        let entropy = flat_entropy.reshape([batch_size, num_agents]);

        // Get value
        let value = self.critic.forward(global_obs);

        (log_probs, entropy, value)
    }
}

/// Rollout buffer for storing trajectories
#[derive(Debug, Clone)]
pub struct RolloutBuffer {
    /// Local observations [timesteps, num_agents, obs_size]
    pub local_obs: Vec<Vec<Vec<f32>>>,
    /// Global observations [timesteps, global_obs_size]
    pub global_obs: Vec<Vec<f32>>,
    /// Action masks [timesteps, num_agents, MAX_ACTIONS]
    pub masks: Vec<Vec<Vec<f32>>>,
    /// Actions taken [timesteps, num_agents]
    pub actions: Vec<Vec<i64>>,
    /// Log probs of actions [timesteps, num_agents]
    pub log_probs: Vec<Vec<f32>>,
    /// Rewards [timesteps] (shared team reward)
    pub rewards: Vec<f32>,
    /// Done flags [timesteps]
    pub dones: Vec<bool>,
    /// Value estimates [timesteps]
    pub values: Vec<f32>,
}

impl RolloutBuffer {
    pub fn new() -> Self {
        Self {
            local_obs: Vec::new(),
            global_obs: Vec::new(),
            masks: Vec::new(),
            actions: Vec::new(),
            log_probs: Vec::new(),
            rewards: Vec::new(),
            dones: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn push(
        &mut self,
        local_obs: Vec<Vec<f32>>,
        global_obs: Vec<f32>,
        masks: Vec<Vec<f32>>,
        actions: Vec<i64>,
        log_probs: Vec<f32>,
        reward: f32,
        done: bool,
        value: f32,
    ) {
        self.local_obs.push(local_obs);
        self.global_obs.push(global_obs);
        self.masks.push(masks);
        self.actions.push(actions);
        self.log_probs.push(log_probs);
        self.rewards.push(reward);
        self.dones.push(done);
        self.values.push(value);
    }

    pub fn len(&self) -> usize {
        self.rewards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rewards.is_empty()
    }

    pub fn clear(&mut self) {
        self.local_obs.clear();
        self.global_obs.clear();
        self.masks.clear();
        self.actions.clear();
        self.log_probs.clear();
        self.rewards.clear();
        self.dones.clear();
        self.values.clear();
    }

    /// Compute returns and advantages using GAE
    pub fn compute_returns_and_advantages(
        &self,
        last_value: f32,
        gamma: f32,
        gae_lambda: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        let n = self.len();
        let mut returns = vec![0.0; n];
        let mut advantages = vec![0.0; n];

        let mut gae = 0.0;
        let mut next_value = last_value;

        for t in (0..n).rev() {
            let not_done = if self.dones[t] { 0.0 } else { 1.0 };
            let delta = self.rewards[t] + gamma * next_value * not_done - self.values[t];
            gae = delta + gamma * gae_lambda * not_done * gae;
            advantages[t] = gae;
            returns[t] = gae + self.values[t];
            next_value = self.values[t];
        }

        (returns, advantages)
    }
}

impl Default for RolloutBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mappo_config_default() {
        let config = MAPPOConfig::default();
        assert_eq!(config.actor_hidden_size, 256);
        assert_eq!(config.critic_hidden_size, 512);
        assert!((config.gamma - 0.99).abs() < 1e-6);
    }

    #[test]
    fn test_rollout_buffer() {
        let mut buffer = RolloutBuffer::new();
        assert!(buffer.is_empty());

        buffer.push(
            vec![vec![0.0; 10]],
            vec![0.0; 20],
            vec![vec![1.0; MAX_ACTIONS]],
            vec![0],
            vec![0.0],
            1.0,
            false,
            0.5,
        );

        assert_eq!(buffer.len(), 1);
    }
}
