//! Reinforcement Learning planner using MAPPO (Multi-Agent PPO with Centralized Critic)
//!
//! This module provides an RL-based alternative to the GOAP planner, using:
//! - Fine-grained action space: dynamically generated action instances with masking
//! - MAPPO: decentralized actors with centralized critic for multi-player coordination
//! - Burn framework with Metal backend for M1 Ultra acceleration
//! - Optional behavioral cloning warmup from GOAP expert trajectories
//!
//! # Architecture
//!
//! ```text
//! WorldState
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  ActionSpace::generate()                                    │
//! │  - Generates all valid action instances per player          │
//! │  - Returns validity mask for action masking                 │
//! └─────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  StateEncoder                                               │
//! │  - Local observations per player (for decentralized actor) │
//! │  - Global observation (for centralized critic)              │
//! └─────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  MAPPO Policy                                               │
//! │  - Actor: local_obs → action_probs (with masking)          │
//! │  - Critic: global_obs → value                               │
//! └─────────────────────────────────────────────────────────────┘
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Executor                                                   │
//! │  - Execute selected actions (multi-tick with CBS)           │
//! │  - Returns DirectedAction per player                        │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// Core modules that don't depend on Burn
pub mod action_space;
pub mod actions;
pub mod encoder;

// Burn-dependent modules
pub mod bc;
pub mod env;
pub mod executor;
pub mod game;
pub mod metrics;
pub mod policy;
pub mod train;

// Re-export commonly used types
pub use action_space::{ActionSpace, MAX_ACTIONS, MultiAgentActionSpace};
pub use actions::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};
pub use bc::{BCConfig, BCTrainer, DemoCollector, DemoDataset, ExpertDemo};
pub use encoder::{EncoderConfig, ObservationBatch, StateEncoder};
pub use env::{BatchEnv, EnvConfig, Observation, RLEnv, StepInfo, StepResult};
pub use executor::{InferenceConfig, RLExecutor};
pub use game::{ComparisonRunner, RLGameRunner};
pub use metrics::{EvaluationMetrics, TrainingMetrics};
pub use policy::{Actor, Critic, MAPPOConfig, MAPPOModel, RolloutBuffer};
pub use train::{PPOTrainer, TrainConfig};
