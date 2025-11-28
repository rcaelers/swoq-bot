//! Metrics and TensorBoard logging for RL training

use std::collections::VecDeque;
use std::path::Path;
use std::time::Instant;

use burn::tensor::backend::Backend;

/// Moving average calculator
#[derive(Debug, Clone)]
pub struct MovingAverage {
    values: VecDeque<f32>,
    window_size: usize,
    sum: f32,
}

impl MovingAverage {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(window_size),
            window_size,
            sum: 0.0,
        }
    }

    pub fn push(&mut self, value: f32) {
        if self.values.len() >= self.window_size {
            if let Some(old) = self.values.pop_front() {
                self.sum -= old;
            }
        }
        self.values.push_back(value);
        self.sum += value;
    }

    pub fn average(&self) -> f32 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f32
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Training metrics tracker
#[derive(Debug)]
pub struct TrainingMetrics {
    /// Episode rewards
    pub episode_rewards: MovingAverage,
    /// Episode lengths
    pub episode_lengths: MovingAverage,
    /// Policy loss
    pub policy_loss: MovingAverage,
    /// Value loss
    pub value_loss: MovingAverage,
    /// Entropy
    pub entropy: MovingAverage,
    /// Level completion rate
    pub completion_rate: MovingAverage,
    /// Deaths per episode
    pub death_rate: MovingAverage,
    /// Current iteration
    pub iteration: usize,
    /// Total timesteps
    pub total_timesteps: usize,
    /// Training start time
    start_time: Instant,
    /// Last log time
    last_log_time: Instant,
}

impl TrainingMetrics {
    pub fn new(window_size: usize) -> Self {
        let now = Instant::now();
        Self {
            episode_rewards: MovingAverage::new(window_size),
            episode_lengths: MovingAverage::new(window_size),
            policy_loss: MovingAverage::new(window_size),
            value_loss: MovingAverage::new(window_size),
            entropy: MovingAverage::new(window_size),
            completion_rate: MovingAverage::new(window_size),
            death_rate: MovingAverage::new(window_size),
            iteration: 0,
            total_timesteps: 0,
            start_time: now,
            last_log_time: now,
        }
    }

    /// Record episode completion
    pub fn record_episode(&mut self, reward: f32, length: usize, completed: bool, died: bool) {
        self.episode_rewards.push(reward);
        self.episode_lengths.push(length as f32);
        self.completion_rate.push(if completed { 1.0 } else { 0.0 });
        self.death_rate.push(if died { 1.0 } else { 0.0 });
    }

    /// Record training losses
    pub fn record_losses(&mut self, policy_loss: f32, value_loss: f32, entropy: f32) {
        self.policy_loss.push(policy_loss);
        self.value_loss.push(value_loss);
        self.entropy.push(entropy);
    }

    /// Update iteration counter
    pub fn update_iteration(&mut self, iteration: usize, timesteps: usize) {
        self.iteration = iteration;
        self.total_timesteps += timesteps;
    }

    /// Get training duration in seconds
    pub fn training_duration_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Get timesteps per second
    pub fn timesteps_per_second(&self) -> f64 {
        let duration = self.training_duration_secs();
        if duration > 0.0 {
            self.total_timesteps as f64 / duration
        } else {
            0.0
        }
    }

    /// Log current metrics to console
    pub fn log_to_console(&mut self) {
        let now = Instant::now();
        let elapsed_since_log = now.duration_since(self.last_log_time).as_secs_f64();

        tracing::info!(
            "Iteration {} | Timesteps {} | SPS {:.1}",
            self.iteration,
            self.total_timesteps,
            self.timesteps_per_second()
        );
        tracing::info!(
            "  Episode: reward={:.2}, length={:.1}, completion={:.1}%, death={:.1}%",
            self.episode_rewards.average(),
            self.episode_lengths.average(),
            self.completion_rate.average() * 100.0,
            self.death_rate.average() * 100.0
        );
        tracing::info!(
            "  Losses: policy={:.4}, value={:.4}, entropy={:.4}",
            self.policy_loss.average(),
            self.value_loss.average(),
            self.entropy.average()
        );

        self.last_log_time = now;
    }
}

impl Default for TrainingMetrics {
    fn default() -> Self {
        Self::new(100)
    }
}

/// TensorBoard logger wrapper
/// Uses file-based logging compatible with TensorBoard
pub struct TensorBoardLogger {
    log_dir: String,
    /// Event file for TensorBoard (simplified implementation)
    step: usize,
}

impl TensorBoardLogger {
    pub fn new(log_dir: &str) -> Self {
        // Create log directory
        std::fs::create_dir_all(log_dir).ok();

        Self {
            log_dir: log_dir.to_string(),
            step: 0,
        }
    }

    /// Log a scalar value
    pub fn log_scalar(&mut self, tag: &str, value: f32, step: usize) {
        self.step = step;
        // In a real implementation, this would write to a TensorBoard event file
        // For now, we'll write to a simple CSV for visualization
        let csv_path = format!("{}/{}.csv", self.log_dir, tag.replace('/', "_"));

        let file_exists = Path::new(&csv_path).exists();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&csv_path)
            .ok();

        if let Some(ref mut f) = file {
            use std::io::Write;
            if !file_exists {
                writeln!(f, "step,value").ok();
            }
            writeln!(f, "{},{}", step, value).ok();
        }
    }

    /// Log multiple scalars at once
    pub fn log_metrics(&mut self, metrics: &TrainingMetrics) {
        let step = metrics.iteration;

        self.log_scalar("episode/reward", metrics.episode_rewards.average(), step);
        self.log_scalar("episode/length", metrics.episode_lengths.average(), step);
        self.log_scalar("episode/completion_rate", metrics.completion_rate.average(), step);
        self.log_scalar("episode/death_rate", metrics.death_rate.average(), step);

        self.log_scalar("losses/policy", metrics.policy_loss.average(), step);
        self.log_scalar("losses/value", metrics.value_loss.average(), step);
        self.log_scalar("losses/entropy", metrics.entropy.average(), step);

        self.log_scalar("performance/timesteps", metrics.total_timesteps as f32, step);
        self.log_scalar("performance/sps", metrics.timesteps_per_second() as f32, step);
    }

    /// Flush and close the logger
    pub fn close(&mut self) {
        // Flush any pending writes
        tracing::info!("TensorBoard logs saved to {}", self.log_dir);
    }
}

/// Evaluation metrics for comparing training modes
#[derive(Debug, Clone, Default)]
pub struct EvaluationMetrics {
    /// Number of evaluation episodes
    pub num_episodes: usize,
    /// Total reward across all episodes
    pub total_reward: f32,
    /// Number of successful completions
    pub num_completions: usize,
    /// Total steps across all episodes
    pub total_steps: usize,
    /// Number of deaths
    pub num_deaths: usize,
    /// Per-level statistics
    pub level_stats: std::collections::HashMap<usize, LevelStats>,
}

#[derive(Debug, Clone, Default)]
pub struct LevelStats {
    pub attempts: usize,
    pub completions: usize,
    pub total_reward: f32,
    pub total_steps: usize,
}

impl EvaluationMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an evaluation episode
    pub fn record_episode(
        &mut self,
        level: usize,
        reward: f32,
        steps: usize,
        completed: bool,
        died: bool,
    ) {
        self.num_episodes += 1;
        self.total_reward += reward;
        self.total_steps += steps;

        if completed {
            self.num_completions += 1;
        }
        if died {
            self.num_deaths += 1;
        }

        let stats = self.level_stats.entry(level).or_default();
        stats.attempts += 1;
        stats.total_reward += reward;
        stats.total_steps += steps;
        if completed {
            stats.completions += 1;
        }
    }

    /// Get average reward
    pub fn avg_reward(&self) -> f32 {
        if self.num_episodes > 0 {
            self.total_reward / self.num_episodes as f32
        } else {
            0.0
        }
    }

    /// Get completion rate
    pub fn completion_rate(&self) -> f32 {
        if self.num_episodes > 0 {
            self.num_completions as f32 / self.num_episodes as f32
        } else {
            0.0
        }
    }

    /// Get average episode length
    pub fn avg_steps(&self) -> f32 {
        if self.num_episodes > 0 {
            self.total_steps as f32 / self.num_episodes as f32
        } else {
            0.0
        }
    }

    /// Get death rate
    pub fn death_rate(&self) -> f32 {
        if self.num_episodes > 0 {
            self.num_deaths as f32 / self.num_episodes as f32
        } else {
            0.0
        }
    }

    /// Print summary
    pub fn print_summary(&self) {
        tracing::info!("=== Evaluation Summary ===");
        tracing::info!("Episodes: {}", self.num_episodes);
        tracing::info!("Avg Reward: {:.2}", self.avg_reward());
        tracing::info!("Completion Rate: {:.1}%", self.completion_rate() * 100.0);
        tracing::info!("Avg Steps: {:.1}", self.avg_steps());
        tracing::info!("Death Rate: {:.1}%", self.death_rate() * 100.0);

        tracing::info!("\nPer-Level Statistics:");
        let mut levels: Vec<_> = self.level_stats.keys().collect();
        levels.sort();
        for &level in &levels {
            let stats = &self.level_stats[level];
            let completion_rate = if stats.attempts > 0 {
                stats.completions as f32 / stats.attempts as f32 * 100.0
            } else {
                0.0
            };
            let avg_reward = if stats.attempts > 0 {
                stats.total_reward / stats.attempts as f32
            } else {
                0.0
            };
            tracing::info!(
                "  Level {}: {} attempts, {:.1}% completion, avg_reward={:.2}",
                level,
                stats.attempts,
                completion_rate,
                avg_reward
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average() {
        let mut avg = MovingAverage::new(3);

        avg.push(1.0);
        assert!((avg.average() - 1.0).abs() < 1e-6);

        avg.push(2.0);
        assert!((avg.average() - 1.5).abs() < 1e-6);

        avg.push(3.0);
        assert!((avg.average() - 2.0).abs() < 1e-6);

        avg.push(4.0); // Pushes out 1.0
        assert!((avg.average() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_evaluation_metrics() {
        let mut metrics = EvaluationMetrics::new();

        metrics.record_episode(1, 10.0, 50, true, false);
        metrics.record_episode(1, 5.0, 100, false, true);

        assert_eq!(metrics.num_episodes, 2);
        assert!((metrics.avg_reward() - 7.5).abs() < 1e-6);
        assert!((metrics.completion_rate() - 0.5).abs() < 1e-6);
    }
}
