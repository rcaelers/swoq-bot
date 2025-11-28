//! RL Game integration - connects RL to the game loop

use std::time::Instant;

use burn::prelude::*;
use burn::tensor::backend::Backend;

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::action_space::ActionSpace;
use super::executor::{InferenceConfig, RLExecutor};
use super::metrics::EvaluationMetrics;

/// Game runner that uses RL for decision making
pub struct RLGameRunner<B: Backend> {
    executor: RLExecutor<B>,
    /// Metrics for evaluation
    metrics: EvaluationMetrics,
    /// Current episode reward
    episode_reward: f32,
    /// Current episode steps
    episode_steps: usize,
    /// Current level
    current_level: usize,
}

impl<B: Backend> RLGameRunner<B> {
    /// Create a new game runner
    pub fn new(device: B::Device, config: InferenceConfig) -> Self {
        let executor = RLExecutor::new(device, config);

        Self {
            executor,
            metrics: EvaluationMetrics::new(),
            episode_reward: 0.0,
            episode_steps: 0,
            current_level: 0,
        }
    }

    /// Load the trained model
    pub fn load_model(&mut self, path: &str) {
        self.executor.load_model(path);
    }

    /// Initialize for a new game
    pub fn init_game(&mut self, world: &WorldState) {
        self.executor.init_players(world.players.len());
        self.episode_reward = 0.0;
        self.episode_steps = 0;
        self.current_level = world.level as usize;
    }

    /// Get actions for the current state
    pub fn get_actions(&mut self, world: &mut WorldState) -> Vec<DirectedAction> {
        self.episode_steps += 1;
        self.executor.step(world)
    }

    /// Get target positions for CBS pathfinding
    pub fn get_targets(&mut self, world: &WorldState) -> Vec<Option<Position>> {
        self.executor.get_target_positions(world)
    }

    /// Record step result
    pub fn record_step(&mut self, reward: f32) {
        self.episode_reward += reward;
    }

    /// Record episode end
    pub fn end_episode(&mut self, completed: bool, died: bool) {
        self.metrics.record_episode(
            self.current_level,
            self.episode_reward,
            self.episode_steps,
            completed,
            died,
        );

        // Reset for next episode
        self.executor.reset();
        self.episode_reward = 0.0;
        self.episode_steps = 0;
    }

    /// Get evaluation metrics
    pub fn metrics(&self) -> &EvaluationMetrics {
        &self.metrics
    }

    /// Print evaluation summary
    pub fn print_summary(&self) {
        self.metrics.print_summary();
    }

    /// Get current action names for debugging
    pub fn get_action_names(&self) -> Vec<String> {
        self.executor.get_action_names()
    }
}

/// Comparison runner for comparing RL vs GOAP performance
pub struct ComparisonRunner {
    /// RL metrics
    rl_metrics: EvaluationMetrics,
    /// GOAP metrics (for comparison)
    goap_metrics: EvaluationMetrics,
}

impl ComparisonRunner {
    pub fn new() -> Self {
        Self {
            rl_metrics: EvaluationMetrics::new(),
            goap_metrics: EvaluationMetrics::new(),
        }
    }

    /// Record RL episode result
    pub fn record_rl_episode(
        &mut self,
        level: usize,
        reward: f32,
        steps: usize,
        completed: bool,
        died: bool,
    ) {
        self.rl_metrics
            .record_episode(level, reward, steps, completed, died);
    }

    /// Record GOAP episode result
    pub fn record_goap_episode(
        &mut self,
        level: usize,
        reward: f32,
        steps: usize,
        completed: bool,
        died: bool,
    ) {
        self.goap_metrics
            .record_episode(level, reward, steps, completed, died);
    }

    /// Print comparison summary
    pub fn print_comparison(&self) {
        tracing::info!("=== RL vs GOAP Comparison ===\n");

        tracing::info!("RL Results:");
        self.rl_metrics.print_summary();

        tracing::info!("\nGOAP Results:");
        self.goap_metrics.print_summary();

        tracing::info!("\n=== Comparison ===");
        tracing::info!(
            "Reward: RL={:.2} vs GOAP={:.2} (diff={:+.2})",
            self.rl_metrics.avg_reward(),
            self.goap_metrics.avg_reward(),
            self.rl_metrics.avg_reward() - self.goap_metrics.avg_reward()
        );
        tracing::info!(
            "Completion: RL={:.1}% vs GOAP={:.1}% (diff={:+.1}%)",
            self.rl_metrics.completion_rate() * 100.0,
            self.goap_metrics.completion_rate() * 100.0,
            (self.rl_metrics.completion_rate() - self.goap_metrics.completion_rate()) * 100.0
        );
        tracing::info!(
            "Steps: RL={:.1} vs GOAP={:.1} (diff={:+.1})",
            self.rl_metrics.avg_steps(),
            self.goap_metrics.avg_steps(),
            self.rl_metrics.avg_steps() - self.goap_metrics.avg_steps()
        );
    }

    /// Save comparison to CSV
    pub fn save_to_csv(&self, path: &str) {
        use std::io::Write;

        let mut file = std::fs::File::create(path).expect("Failed to create comparison file");

        writeln!(file, "metric,rl,goap,difference").ok();
        writeln!(
            file,
            "avg_reward,{:.4},{:.4},{:.4}",
            self.rl_metrics.avg_reward(),
            self.goap_metrics.avg_reward(),
            self.rl_metrics.avg_reward() - self.goap_metrics.avg_reward()
        )
        .ok();
        writeln!(
            file,
            "completion_rate,{:.4},{:.4},{:.4}",
            self.rl_metrics.completion_rate(),
            self.goap_metrics.completion_rate(),
            self.rl_metrics.completion_rate() - self.goap_metrics.completion_rate()
        )
        .ok();
        writeln!(
            file,
            "avg_steps,{:.4},{:.4},{:.4}",
            self.rl_metrics.avg_steps(),
            self.goap_metrics.avg_steps(),
            self.rl_metrics.avg_steps() - self.goap_metrics.avg_steps()
        )
        .ok();
        writeln!(
            file,
            "death_rate,{:.4},{:.4},{:.4}",
            self.rl_metrics.death_rate(),
            self.goap_metrics.death_rate(),
            self.rl_metrics.death_rate() - self.goap_metrics.death_rate()
        )
        .ok();

        tracing::info!("Comparison saved to {}", path);
    }
}

impl Default for ComparisonRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility to analyze action distribution
pub fn analyze_action_distribution(world: &WorldState) {
    use super::actions::ActionType;

    tracing::info!("=== Action Distribution Analysis ===\n");

    for player_idx in 0..world.players.len() {
        let action_space = ActionSpace::generate(world, player_idx);
        let counts = action_space.action_type_counts();

        tracing::info!("Player {}:", player_idx);
        for (type_idx, count) in counts.iter().enumerate() {
            if *count > 0 {
                if let Some(action_type) = ActionType::from_index(type_idx) {
                    tracing::info!("  {:?}: {}", action_type, count);
                }
            }
        }
        tracing::info!("  Total valid actions: {}", action_space.num_valid());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_runner() {
        let mut runner = ComparisonRunner::new();

        runner.record_rl_episode(1, 10.0, 50, true, false);
        runner.record_goap_episode(1, 8.0, 60, true, false);

        assert_eq!(runner.rl_metrics.num_episodes, 1);
        assert_eq!(runner.goap_metrics.num_episodes, 1);
    }
}
