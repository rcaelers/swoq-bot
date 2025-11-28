use crate::infra::{Color, Position};
use crate::planners::goap::game_state::{PlanningState, ResourceClaim};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

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
    pub exit_pos: Option<Position>, // Position to move to after leaving the plate
    pub other_player_target: Option<Position>, // Target position of the other player (captured during prepare)
    phase: WaitOnPlatePhase,                   // Current execution phase
}

impl WaitOnPlateAction {}

impl GOAPActionTrait for WaitOnPlateAction {
    fn precondition(&self, world: &WorldState, state: &PlanningState, player_index: usize) -> bool {
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

        // Check if this color's pressure plate is already claimed by another player
        let claim = ResourceClaim::PressurePlate(self.color);
        let already_claimed = state
            .resource_claims
            .get(&claim)
            .is_some_and(|&claimer| claimer != player_index);

        plate_exists && reachable && !already_claimed
    }

    fn effect_start(
        &self,
        _world: &mut WorldState,
        state: &mut PlanningState,
        player_index: usize,
    ) {
        // Claim this pressure plate color to prevent other players from targeting it
        let claim = ResourceClaim::PressurePlate(self.color);
        state.resource_claims.insert(claim, player_index);
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].position = self.plate_pos;
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
                // Check if other player reached target
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
            WaitOnPlatePhase::MovingOff => {
                // Stay in this phase until complete
            }
        }

        // Return destination based on phase
        match self.phase {
            WaitOnPlatePhase::MovingTo => world
                .find_path(player_pos, self.plate_pos)
                .map(|_| self.plate_pos),
            WaitOnPlatePhase::Waiting => {
                // Stay on plate, no movement needed
                Some(self.plate_pos)
            }
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
                // Move to the plate using CBS path
                execute_move_to(world, player_index, self.plate_pos, execution_state)
            }
            WaitOnPlatePhase::Waiting => {
                // Just wait - phase transition happens in prepare()
                (DirectedAction::None, ExecutionStatus::InProgress)
            }
            WaitOnPlatePhase::MovingOff => {
                if let Some(exit_pos) = self.exit_pos {
                    if player_pos != exit_pos {
                        // Move to exit using CBS path
                        execute_move_to(world, player_index, exit_pos, execution_state)
                    } else {
                        // Reached exit position - complete
                        world.players[player_index].current_path = None;
                        world.players[player_index].current_destination = None;
                        (DirectedAction::None, ExecutionStatus::Complete)
                    }
                } else {
                    // No exit_pos - complete immediately
                    world.players[player_index].current_path = None;
                    world.players[player_index].current_destination = None;
                    (DirectedAction::None, ExecutionStatus::Complete)
                }
            }
        }
    }

    fn cost(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> f32 {
        5.0 + world
            .path_distance(world.players[player_index].position, self.plate_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> u32 {
        // Distance to plate + time waiting (for synchronized multi-player actions)
        // We estimate the synchronized partner will need this long
        let distance = world
            .path_distance(world.players[player_index].position, self.plate_pos)
            .unwrap_or(1000) as u32;
        distance + 5 // +5 ticks for partner to complete their part
    }

    fn name(&self) -> String {
        "WaitOnPlate".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();

        // Get other player's position for reachability check
        let other_player_index = 1 - player_index;
        let other_player_pos = if other_player_index < world.players.len() {
            Some(world.players[other_player_index].position)
        } else {
            None
        };

        // Simple check: if both players can reach each other, they're on the same side
        // and don't need door coordination
        let player_pos = world.players[player_index].position;
        let players_can_reach_each_other = if let Some(other_pos) = other_player_pos {
            let p = world.find_path(player_pos, other_pos).is_some();
            tracing::trace!(
                player_index = player_index,
                other_player_index = other_player_index,
                players_can_reach_each_other = p,
                "WaitOnPlateAction reachability check"
            );
            p
        } else {
            false
        };

        // Only skip if players can reach each other AND all exploration is complete
        // If there are unexplored areas, we might need plate coordination to access them
        let player = &world.players[player_index];
        let has_unexplored = !player.unexplored_frontier.is_empty();
        if !players_can_reach_each_other || has_unexplored {
            tracing::trace!(
                player_index = player_index,
                "Skipping WaitOnPlateAction - players can reach each other and unexplored areas"
            );
            return actions;
        }

        for color in [
            crate::infra::Color::Red,
            crate::infra::Color::Green,
            crate::infra::Color::Blue,
        ] {
            if let Some(plate_positions) = world.pressure_plates.get_positions(color) {
                for plate_pos in plate_positions {
                    // Skip if this player can't reach this plate
                    if world.find_path(player_pos, *plate_pos).is_none() {
                        continue;
                    }
                    // Find an exit position (walkable neighbor of the plate)
                    let exit_pos = plate_pos
                        .neighbors()
                        .into_iter()
                        .find(|pos| world.is_walkable(pos, None));

                    let action = WaitOnPlateAction {
                        color,
                        plate_pos: *plate_pos,
                        exit_pos,
                        other_player_target: None, // Captured during prepare()
                        phase: WaitOnPlatePhase::MovingTo,
                    };
                    if action.precondition(world, state, player_index) {
                        actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
