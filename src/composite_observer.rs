use crate::game_observer::GameObserver;
use crate::goals::Goal;
use crate::swoq_interface::{ActResult, DirectedAction, GameStatus, State};
use crate::world_state::WorldState;

pub struct CompositeObserver {
    observers: Vec<Box<dyn GameObserver>>,
}

impl CompositeObserver {
    pub fn new(observers: Vec<Box<dyn GameObserver>>) -> Self {
        Self { observers }
    }
}

impl GameObserver for CompositeObserver {
    fn on_game_start(
        &mut self,
        game_id: &str,
        seed: Option<i32>,
        map_width: i32,
        map_height: i32,
        visibility_range: i32,
    ) {
        for observer in &mut self.observers {
            observer.on_game_start(game_id, seed, map_width, map_height, visibility_range);
        }
    }

    fn on_new_level(&mut self, level: i32) {
        for observer in &mut self.observers {
            observer.on_new_level(level);
        }
    }

    fn on_state_update(
        &mut self,
        state: &State,
        world: &WorldState,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    ) {
        for observer in &mut self.observers {
            observer.on_state_update(state, world, game_count, successful_runs, failed_runs);
        }
    }

    fn on_goal_selected(&mut self, goal: &Goal, world: &WorldState) {
        for observer in &mut self.observers {
            observer.on_goal_selected(goal, world);
        }
    }

    fn on_action_selected(&mut self, action: DirectedAction, world: &WorldState) {
        for observer in &mut self.observers {
            observer.on_action_selected(action, world);
        }
    }

    fn on_action_result(
        &mut self,
        action: DirectedAction,
        action2: Option<DirectedAction>,
        result: ActResult,
        world: &WorldState,
    ) {
        for observer in &mut self.observers {
            observer.on_action_result(action, action2, result, world);
        }
    }

    fn on_game_finished(
        &mut self,
        status: GameStatus,
        final_tick: i32,
        game_count: i32,
        successful_runs: i32,
        failed_runs: i32,
    ) {
        for observer in &mut self.observers {
            observer.on_game_finished(status, final_tick, game_count, successful_runs, failed_runs);
        }
    }
}
