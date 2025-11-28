//! WaitOnPlate action - stand on a pressure plate for cooperative gameplay

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitOnPlatePhase {
    MovingTo,
    Waiting,
    MovingOff,
}

#[derive(Debug, Clone)]
pub struct WaitOnPlateAction {
    pub color: Color,
    pub plate_pos: Position,
    pub exit_pos: Option<Position>,
    pub other_player_target: Option<Position>,
    phase: WaitOnPlatePhase,
}

impl RLActionTrait for WaitOnPlateAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        // WaitOnPlateAction is only for two-player mode
        if !world.is_two_player_mode() {
            return false;
        }

        let player = &world.players[player_index];
        let plate_exists = world
            .pressure_plates
            .get_positions(self.color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));
        let reachable = world.find_path(player.position, self.plate_pos).is_some();

        plate_exists && reachable
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player_pos = world.players[player_index].position;

        // Capture the other player's target position
        let other_player_index = 1 - player_index;
        if other_player_index < world.players.len() {
            self.other_player_target = world.players[other_player_index].coop_door_target;
        }

        // Update phase based on current position
        match self.phase {
            WaitOnPlatePhase::MovingTo => {
                if player_pos == self.plate_pos {
                    self.phase = WaitOnPlatePhase::Waiting;
                }
            }
            WaitOnPlatePhase::Waiting => {
                if other_player_index < world.players.len() {
                    let other_player = &world.players[other_player_index];
                    if self
                        .other_player_target
                        .is_some_and(|target_pos| other_player.position == target_pos)
                    {
                        self.phase = WaitOnPlatePhase::MovingOff;
                    }
                }
            }
            WaitOnPlatePhase::MovingOff => {}
        }

        match self.phase {
            WaitOnPlatePhase::MovingTo => world
                .find_path(player_pos, self.plate_pos)
                .map(|_| self.plate_pos),
            WaitOnPlatePhase::Waiting => Some(self.plate_pos),
            WaitOnPlatePhase::MovingOff => self.exit_pos,
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player_pos = world.players[player_index].position;

        match self.phase {
            WaitOnPlatePhase::MovingTo => {
                execute_move_to(world, player_index, self.plate_pos, execution_state)
            }
            WaitOnPlatePhase::Waiting => (DirectedAction::None, ExecutionStatus::InProgress),
            WaitOnPlatePhase::MovingOff => {
                if let Some(exit_pos) = self.exit_pos {
                    if player_pos != exit_pos {
                        execute_move_to(world, player_index, exit_pos, execution_state)
                    } else {
                        world.players[player_index].current_path = None;
                        world.players[player_index].current_destination = None;
                        (DirectedAction::None, ExecutionStatus::Complete)
                    }
                } else {
                    world.players[player_index].current_path = None;
                    world.players[player_index].current_destination = None;
                    (DirectedAction::None, ExecutionStatus::Complete)
                }
            }
        }
    }

    fn name(&self) -> String {
        "WaitOnPlate".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::WaitOnPlate as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.plate_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();

        let other_player_index = 1 - player_index;
        let other_player_pos = if other_player_index < world.players.len() {
            Some(world.players[other_player_index].position)
        } else {
            None
        };

        let player_pos = world.players[player_index].position;
        let players_can_reach_each_other = if let Some(other_pos) = other_player_pos {
            world.find_path(player_pos, other_pos).is_some()
        } else {
            false
        };

        let player = &world.players[player_index];
        let has_unexplored = !player.unexplored_frontier.is_empty();
        if players_can_reach_each_other && has_unexplored {
            return actions;
        }

        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(plate_positions) = world.pressure_plates.get_positions(color) {
                for plate_pos in plate_positions {
                    if world.find_path(player_pos, *plate_pos).is_none() {
                        continue;
                    }

                    let exit_pos = plate_pos
                        .neighbors()
                        .into_iter()
                        .find(|pos| world.is_walkable(pos, None));

                    let action = WaitOnPlateAction {
                        color,
                        plate_pos: *plate_pos,
                        exit_pos,
                        other_player_target: None,
                        phase: WaitOnPlatePhase::MovingTo,
                    };
                    if action.precondition(world, player_index) {
                        actions.push(Box::new(action) as Box<dyn RLActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
