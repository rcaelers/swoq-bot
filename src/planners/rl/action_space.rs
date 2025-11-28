//! Action space management for RL - handles dynamic action enumeration and masking

use crate::infra::Position;
use crate::state::WorldState;

use super::actions::{
    ActionType, AttackEnemyAction, AvoidEnemyAction, DropBoulderAction, DropBoulderOnPlateAction,
    ExploreAction, GetKeyAction, HuntEnemyAction, OpenDoorAction, PassThroughDoorWithPlateAction,
    PickupBoulderAction, PickupHealthAction, PickupSwordAction, RLActionTrait, ReachExitAction,
    TouchPlateAction, WaitAction, WaitOnPlateAction,
};

/// Maximum number of actions in the action space (padded with no-ops for masking)
pub const MAX_ACTIONS: usize = 128;

/// A single action instance with its metadata
#[derive(Debug, Clone)]
pub struct ActionInstance {
    /// The action itself
    pub action: Box<dyn RLActionTrait>,
    /// Global index in the padded action space
    pub index: usize,
    /// Whether this action is valid (for masking)
    pub valid: bool,
}

/// Manages the action space for a single player
#[derive(Debug)]
pub struct ActionSpace {
    /// All possible actions for this timestep (padded to MAX_ACTIONS)
    actions: Vec<Option<Box<dyn RLActionTrait>>>,
    /// Validity mask (true = valid action, false = masked/no-op)
    mask: Vec<bool>,
    /// Number of valid actions
    num_valid: usize,
}

impl ActionSpace {
    /// Generate the action space for a player at the current state
    pub fn generate(world: &WorldState, player_index: usize) -> Self {
        let mut actions: Vec<Option<Box<dyn RLActionTrait>>> = Vec::with_capacity(MAX_ACTIONS);
        let mut mask = vec![false; MAX_ACTIONS];
        let mut num_valid = 0;

        // Generate all possible action instances for each action type
        let mut generated_actions = Vec::new();

        // Generate actions in order of ActionType
        generated_actions.extend(ExploreAction::generate(world, player_index));
        generated_actions.extend(GetKeyAction::generate(world, player_index));
        generated_actions.extend(OpenDoorAction::generate(world, player_index));
        generated_actions.extend(PickupSwordAction::generate(world, player_index));
        generated_actions.extend(PickupHealthAction::generate(world, player_index));
        generated_actions.extend(AttackEnemyAction::generate(world, player_index));
        generated_actions.extend(HuntEnemyAction::generate(world, player_index));
        generated_actions.extend(AvoidEnemyAction::generate(world, player_index));
        generated_actions.extend(WaitOnPlateAction::generate(world, player_index));
        generated_actions.extend(PassThroughDoorWithPlateAction::generate(world, player_index));
        generated_actions.extend(PickupBoulderAction::generate(world, player_index));
        generated_actions.extend(DropBoulderAction::generate(world, player_index));
        generated_actions.extend(DropBoulderOnPlateAction::generate(world, player_index));
        generated_actions.extend(TouchPlateAction::generate(world, player_index));
        generated_actions.extend(ReachExitAction::generate(world, player_index));
        generated_actions.extend(WaitAction::generate(world, player_index));

        // Fill in the action space with valid actions
        for action in generated_actions {
            if num_valid < MAX_ACTIONS {
                actions.push(Some(action));
                mask[num_valid] = true;
                num_valid += 1;
            }
        }

        // Pad with None for remaining slots
        while actions.len() < MAX_ACTIONS {
            actions.push(None);
        }

        Self {
            actions,
            mask,
            num_valid,
        }
    }

    /// Get the validity mask as a slice of bools
    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// Get the validity mask as a float array (1.0 = valid, 0.0 = invalid)
    pub fn mask_as_floats(&self) -> Vec<f32> {
        self.mask
            .iter()
            .map(|&v| if v { 1.0 } else { 0.0 })
            .collect()
    }

    /// Get the number of valid actions
    pub fn num_valid(&self) -> usize {
        self.num_valid
    }

    /// Check if any actions are available
    pub fn has_actions(&self) -> bool {
        self.num_valid > 0
    }

    /// Get an action by index
    pub fn get_action(&self, index: usize) -> Option<&Box<dyn RLActionTrait>> {
        if index < MAX_ACTIONS && self.mask[index] {
            self.actions[index].as_ref()
        } else {
            None
        }
    }

    /// Take an action by index (moves the action out)
    pub fn take_action(&mut self, index: usize) -> Option<Box<dyn RLActionTrait>> {
        if index < MAX_ACTIONS && self.mask[index] {
            self.mask[index] = false;
            self.num_valid = self.num_valid.saturating_sub(1);
            self.actions[index].take()
        } else {
            None
        }
    }

    /// Get a cloned action by index
    pub fn clone_action(&self, index: usize) -> Option<Box<dyn RLActionTrait>> {
        if index < MAX_ACTIONS && self.mask[index] {
            self.actions[index].as_ref().map(|a| a.clone_box())
        } else {
            None
        }
    }

    /// Iterate over valid actions with their indices
    pub fn iter_valid(&self) -> impl Iterator<Item = (usize, &Box<dyn RLActionTrait>)> {
        self.actions
            .iter()
            .enumerate()
            .filter(|(i, _)| self.mask[*i])
            .filter_map(|(i, a)| a.as_ref().map(|action| (i, action)))
    }

    /// Get action type counts for debugging/logging
    pub fn action_type_counts(&self) -> [usize; ActionType::COUNT] {
        let mut counts = [0; ActionType::COUNT];
        for (_, action) in self.iter_valid() {
            let type_idx = action.action_type_index();
            if type_idx < ActionType::COUNT {
                counts[type_idx] += 1;
            }
        }
        counts
    }
}

/// Encodes action instances for the neural network
#[derive(Debug, Clone)]
pub struct ActionEncoder {
    /// Maximum map width for position encoding
    pub max_width: usize,
    /// Maximum map height for position encoding
    pub max_height: usize,
}

impl ActionEncoder {
    pub fn new(max_width: usize, max_height: usize) -> Self {
        Self {
            max_width,
            max_height,
        }
    }

    /// Encode a single action as a feature vector
    /// Features: [action_type_one_hot (16), target_x_norm, target_y_norm, has_target]
    pub fn encode_action(&self, action: &Box<dyn RLActionTrait>) -> Vec<f32> {
        let mut features = vec![0.0; Self::action_feature_size()];

        // One-hot encode action type
        let type_idx = action.action_type_index();
        if type_idx < ActionType::COUNT {
            features[type_idx] = 1.0;
        }

        // Encode target position
        if let Some(pos) = action.target_position() {
            let base = ActionType::COUNT;
            features[base] = pos.x as f32 / self.max_width as f32;
            features[base + 1] = pos.y as f32 / self.max_height as f32;
            features[base + 2] = 1.0; // has_target flag
        }

        features
    }

    /// Get the size of action feature vector
    pub const fn action_feature_size() -> usize {
        ActionType::COUNT + 3 // one-hot + (x, y, has_target)
    }

    /// Encode all actions in the action space
    pub fn encode_action_space(&self, action_space: &ActionSpace) -> Vec<Vec<f32>> {
        let mut encoded = Vec::with_capacity(MAX_ACTIONS);

        for i in 0..MAX_ACTIONS {
            if let Some(action) = action_space.get_action(i) {
                encoded.push(self.encode_action(action));
            } else {
                // Pad with zeros for invalid actions
                encoded.push(vec![0.0; Self::action_feature_size()]);
            }
        }

        encoded
    }
}

/// Multi-agent action space manager
pub struct MultiAgentActionSpace {
    /// Action spaces for each player
    pub player_spaces: Vec<ActionSpace>,
}

impl MultiAgentActionSpace {
    /// Generate action spaces for all players
    pub fn generate(world: &WorldState) -> Self {
        let player_spaces = (0..world.players.len())
            .map(|i| ActionSpace::generate(world, i))
            .collect();

        Self { player_spaces }
    }

    /// Get all masks as a 2D array [num_players, MAX_ACTIONS]
    pub fn all_masks(&self) -> Vec<Vec<f32>> {
        self.player_spaces
            .iter()
            .map(|s| s.mask_as_floats())
            .collect()
    }

    /// Check if all players have at least one action
    pub fn all_have_actions(&self) -> bool {
        self.player_spaces.iter().all(|s| s.has_actions())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_encoder_feature_size() {
        assert_eq!(ActionEncoder::action_feature_size(), 19); // 16 types + 3 position features
    }

    #[test]
    fn test_action_space_max_size() {
        assert_eq!(MAX_ACTIONS, 128);
    }
}
