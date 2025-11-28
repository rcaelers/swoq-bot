//! Wait action - do nothing for a specified duration

use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Clone, Debug)]
pub struct WaitAction {
    duration: u32,
}

impl WaitAction {
    pub fn new(duration: u32) -> Self {
        Self { duration }
    }
}

impl RLActionTrait for WaitAction {
    fn precondition(&self, _world: &WorldState, _player_index: usize) -> bool {
        true // Can always wait as fallback
    }

    fn execute(
        &self,
        _world: &mut WorldState,
        _player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        (DirectedAction::None, ExecutionStatus::Complete)
    }

    fn name(&self) -> String {
        format!("Wait({})", self.duration)
    }

    fn action_type_index(&self) -> usize {
        ActionType::Wait as usize
    }

    fn generate(_world: &WorldState, _player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        // Return a default 1-tick wait
        vec![Box::new(WaitAction::new(1))]
    }
}
