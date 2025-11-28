//! Behavioral Cloning warmup for RL - learn from GOAP expert trajectories

use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::*;
use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};
use burn::tensor::backend::AutodiffBackend;
use rand::seq::IndexedRandom;

use super::action_space::MAX_ACTIONS;
use super::encoder::EncoderConfig;
use super::policy::{MAPPOConfig, MAPPOModel};

/// Configuration for behavioral cloning
#[derive(Debug, Clone)]
pub struct BCConfig {
    /// Number of training epochs
    pub epochs: usize,
    /// Batch size
    pub batch_size: usize,
    /// Learning rate
    pub learning_rate: f64,
    /// Path to save the trained model
    pub save_path: String,
    /// Log frequency (batches)
    pub log_freq: usize,
}

impl Default for BCConfig {
    fn default() -> Self {
        Self {
            epochs: 100,
            batch_size: 64,
            learning_rate: 1e-3,
            save_path: "bc_model".to_string(),
            log_freq: 100,
        }
    }
}

/// A single expert demonstration
#[derive(Debug, Clone)]
pub struct ExpertDemo {
    /// Local observations [num_agents, obs_size]
    pub local_obs: Vec<Vec<f32>>,
    /// Global observation [global_obs_size]
    pub global_obs: Vec<f32>,
    /// Action masks [num_agents, MAX_ACTIONS]
    pub action_masks: Vec<Vec<f32>>,
    /// Expert action indices [num_agents]
    pub actions: Vec<i64>,
}

/// Dataset of expert demonstrations
pub struct DemoDataset {
    demos: Vec<ExpertDemo>,
}

impl DemoDataset {
    pub fn new() -> Self {
        Self { demos: Vec::new() }
    }

    pub fn add(&mut self, demo: ExpertDemo) {
        self.demos.push(demo);
    }

    pub fn len(&self) -> usize {
        self.demos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.demos.is_empty()
    }

    /// Get a random batch of demos
    pub fn sample_batch(&self, batch_size: usize) -> Vec<&ExpertDemo> {
        let mut rng = rand::rng();
        let batch_size = batch_size.min(self.demos.len());
        
        // Use IndexedRandom trait for choose_multiple
        self.demos.choose_multiple(&mut rng, batch_size).collect()
    }

    /// Iterate over all demos
    pub fn iter(&self) -> impl Iterator<Item = &ExpertDemo> {
        self.demos.iter()
    }
}

impl Default for DemoDataset {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavioral Cloning trainer
pub struct BCTrainer<B: AutodiffBackend> {
    model: MAPPOModel<B>,
    config: BCConfig,
    device: B::Device,
}

impl<B: AutodiffBackend> BCTrainer<B> {
    pub fn new(
        device: B::Device,
        encoder_config: &EncoderConfig,
        mappo_config: &MAPPOConfig,
        config: BCConfig,
    ) -> Self {
        let model = MAPPOModel::new(&device, encoder_config, mappo_config);

        Self {
            model,
            config,
            device,
        }
    }

    /// Train the model on expert demonstrations
    pub fn train(&mut self, dataset: &DemoDataset) -> f32 {
        if dataset.is_empty() {
            tracing::warn!("Empty demonstration dataset, skipping BC training");
            return 0.0;
        }

        tracing::info!("Starting BC training with {} demonstrations", dataset.len());

        // Create optimizer
        let optim_config = AdamConfig::new();
        let mut optimizer = optim_config.init::<B, MAPPOModel<B>>();

        let mut total_loss = 0.0;
        let mut num_batches = 0;

        for epoch in 0..self.config.epochs {
            let mut epoch_loss = 0.0;
            let mut epoch_batches = 0;

            // Process in batches
            let num_batches_per_epoch =
                (dataset.len() + self.config.batch_size - 1) / self.config.batch_size;

            for batch_idx in 0..num_batches_per_epoch {
                let batch = dataset.sample_batch(self.config.batch_size);

                if batch.is_empty() {
                    continue;
                }

                let loss = self.train_batch(&batch, &mut optimizer);
                epoch_loss += loss;
                epoch_batches += 1;
                total_loss += loss;
                num_batches += 1;

                if num_batches % self.config.log_freq == 0 {
                    tracing::info!("Epoch {}, Batch {}: loss = {:.4}", epoch, batch_idx, loss);
                }
            }

            let avg_epoch_loss = if epoch_batches > 0 {
                epoch_loss / epoch_batches as f32
            } else {
                0.0
            };

            tracing::info!("Epoch {} complete: avg_loss = {:.4}", epoch, avg_epoch_loss);
        }

        // Save the trained model
        self.save_model(&self.config.save_path);

        let avg_loss = if num_batches > 0 {
            total_loss / num_batches as f32
        } else {
            0.0
        };

        tracing::info!("BC training complete: final_avg_loss = {:.4}", avg_loss);
        avg_loss
    }

    /// Train on a single batch
    fn train_batch<O: Optimizer<MAPPOModel<B>, B>>(
        &mut self, 
        batch: &[&ExpertDemo],
        optimizer: &mut O,
    ) -> f32 {
        let batch_size = batch.len();
        if batch_size == 0 {
            return 0.0;
        }

        let num_agents = batch[0].local_obs.len();
        let obs_size = batch[0].local_obs[0].len();

        // Collect batch data
        let local_obs: Vec<f32> = batch
            .iter()
            .flat_map(|d| d.local_obs.iter().flat_map(|a| a.iter().copied()))
            .collect();
        let global_obs: Vec<f32> = batch
            .iter()
            .flat_map(|d| d.global_obs.iter().copied())
            .collect();
        let masks: Vec<f32> = batch
            .iter()
            .flat_map(|d| d.action_masks.iter().flat_map(|a| a.iter().copied()))
            .collect();
        let actions: Vec<i64> = batch
            .iter()
            .flat_map(|d| d.actions.iter().copied())
            .collect();

        // Create tensors
        let local_obs_tensor: Tensor<B, 3> =
            Tensor::<B, 1>::from_floats(local_obs.as_slice(), &self.device)
                .reshape([batch_size, num_agents, obs_size]);
        let global_obs_tensor: Tensor<B, 2> =
            Tensor::<B, 1>::from_floats(global_obs.as_slice(), &self.device)
                .reshape([batch_size, batch[0].global_obs.len()]);
        let masks_tensor: Tensor<B, 3> = 
            Tensor::<B, 1>::from_floats(masks.as_slice(), &self.device)
                .reshape([batch_size, num_agents, MAX_ACTIONS]);
        let actions_tensor: Tensor<B, 2, Int> =
            Tensor::<B, 1, Int>::from_ints(actions.as_slice(), &self.device)
                .reshape([batch_size, num_agents]);

        // Forward pass
        let (log_probs, _entropy, _values) = self.model.evaluate_actions(
            local_obs_tensor,
            global_obs_tensor,
            masks_tensor,
            actions_tensor,
        );

        // Cross-entropy loss (negative log probability of expert actions)
        let loss = -log_probs.mean();

        // Backward pass and update
        let grads = loss.backward();
        let grads = GradientsParams::from_grads(grads, &self.model);
        self.model = optimizer.step(self.config.learning_rate, self.model.clone(), grads);

        loss.into_data().to_vec::<f32>().unwrap()[0]
    }

    /// Save the trained model
    pub fn save_model(&self, path: &str) {
        let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
        self.model
            .clone()
            .save_file(path, &recorder)
            .expect("Failed to save BC model");
        tracing::info!("Saved BC model to {}", path);
    }

    /// Get the trained model (consumes the trainer)
    pub fn into_model(self) -> MAPPOModel<B> {
        self.model
    }

    /// Get a reference to the model
    pub fn model(&self) -> &MAPPOModel<B> {
        &self.model
    }
}

/// Collector for expert demonstrations from GOAP
pub struct DemoCollector {
    dataset: DemoDataset,
}

impl DemoCollector {
    pub fn new() -> Self {
        Self {
            dataset: DemoDataset::new(),
        }
    }

    /// Add a demonstration from GOAP execution
    pub fn add_demo(
        &mut self,
        local_obs: Vec<Vec<f32>>,
        global_obs: Vec<f32>,
        action_masks: Vec<Vec<f32>>,
        actions: Vec<i64>,
    ) {
        self.dataset.add(ExpertDemo {
            local_obs,
            global_obs,
            action_masks,
            actions,
        });
    }

    /// Get the collected dataset
    pub fn into_dataset(self) -> DemoDataset {
        self.dataset
    }

    /// Get a reference to the dataset
    pub fn dataset(&self) -> &DemoDataset {
        &self.dataset
    }

    /// Get the number of collected demos
    pub fn len(&self) -> usize {
        self.dataset.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dataset.is_empty()
    }
}

impl Default for DemoCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bc_config_default() {
        let config = BCConfig::default();
        assert_eq!(config.epochs, 100);
        assert_eq!(config.batch_size, 64);
    }

    #[test]
    fn test_demo_dataset() {
        let mut dataset = DemoDataset::new();
        assert!(dataset.is_empty());

        dataset.add(ExpertDemo {
            local_obs: vec![vec![0.0; 10]],
            global_obs: vec![0.0; 20],
            action_masks: vec![vec![1.0; MAX_ACTIONS]],
            actions: vec![0],
        });

        assert_eq!(dataset.len(), 1);
        assert!(!dataset.is_empty());
    }
}
