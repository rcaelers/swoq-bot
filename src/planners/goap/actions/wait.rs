use crate::planners::goap::actions::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};
use crate::planners::goap::game_state::PlanningState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

#[derive(Clone, Debug)]
pub struct WaitAction {
    duration: u32,
}

impl WaitAction {
    pub fn new(duration: u32) -> Self {
        Self { duration }
    }
}

impl GOAPActionTrait for WaitAction {
    fn name(&self) -> String {
        format!("Wait({})", self.duration)
    }

    fn precondition(
        &self,
        _world: &WorldState,
        _state: &PlanningState,
        _player_index: usize,
    ) -> bool {
        true // Can always wait as fallback
    }

    fn effect_start(
        &self,
        _world: &mut WorldState,
        _state: &mut PlanningState,
        _player_index: usize,
    ) {
        // No state changes
    }

    fn effect_end(
        &self,
        _world: &mut WorldState,
        _state: &mut PlanningState,
        _player_index: usize,
    ) {
        // No state changes - player just waits
    }

    fn cost(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        1.0
    }

    fn duration(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> u32 {
        self.duration
    }

    fn reward(&self, _world: &WorldState, _state: &PlanningState, _player_index: usize) -> f32 {
        0.0 // No reward for waiting
    }

    fn execute(
        &self,
        _world: &mut WorldState,
        _player_index: usize,
        _execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        (DirectedAction::None, ExecutionStatus::Complete)
    }

    fn generate(
        _world: &WorldState,
        _state: &PlanningState,
        _player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>>
    where
        Self: Sized,
    {
        // This will be called from planner with context
        // For now, return a default 1-tick wait
        vec![Box::new(WaitAction::new(1))]
    }
}
