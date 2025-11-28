//! PassThroughDoorWithPlate action - coordinate with another player to pass through a door

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct PassThroughDoorWithPlateAction {
    pub door_color: Color,
    pub door_pos: Position,
    pub wait_pos: Position,
    pub target_pos: Position,
    pub plate_pos: Position,
}

impl RLActionTrait for PassThroughDoorWithPlateAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        // Only for two-player mode
        if !world.is_two_player_mode() {
            return false;
        }

        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));

        door_exists && plate_exists
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player_pos = world.players[player_index].position;

        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));

        if !door_exists || !plate_exists {
            return None;
        }

        // Store our target position so WaitOnPlateAction can read it
        world.players[player_index].coop_door_target = Some(self.target_pos);

        // If door is already open, path directly to the target
        if world.is_door_open(self.door_color)
            && world.find_path(player_pos, self.target_pos).is_some()
        {
            return Some(self.target_pos);
        }

        // Door is closed - go to wait_pos
        if world.find_path(player_pos, self.wait_pos).is_some() {
            Some(self.wait_pos)
        } else {
            None
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player_pos = world.players[player_index].position;

        // Check if we're at the target position
        if player_pos == self.target_pos {
            let other_player_index = 1 - player_index;
            if other_player_index < world.players.len() {
                let other_player = &world.players[other_player_index];

                if other_player.position == self.plate_pos {
                    return (DirectedAction::None, ExecutionStatus::InProgress);
                }
            }

            world.players[player_index].current_path = None;
            world.players[player_index].current_destination = None;
            world.players[player_index].coop_door_target = None;
            return (DirectedAction::None, ExecutionStatus::Complete);
        }

        // Check if we're on the door tile itself
        if player_pos == self.door_pos {
            let dx = self.target_pos.x - player_pos.x;
            let dy = self.target_pos.y - player_pos.y;

            let action = if dy < 0 {
                DirectedAction::MoveNorth
            } else if dy > 0 {
                DirectedAction::MoveSouth
            } else if dx > 0 {
                DirectedAction::MoveEast
            } else if dx < 0 {
                DirectedAction::MoveWest
            } else {
                DirectedAction::None
            };

            return (action, ExecutionStatus::InProgress);
        }

        // Check if we're adjacent to the door
        if player_pos.is_adjacent(&self.door_pos) {
            if world.is_door_open(self.door_color) {
                let dx = self.door_pos.x - player_pos.x;
                let dy = self.door_pos.y - player_pos.y;

                let action = if dy < 0 {
                    DirectedAction::MoveNorth
                } else if dy > 0 {
                    DirectedAction::MoveSouth
                } else if dx > 0 {
                    DirectedAction::MoveEast
                } else if dx < 0 {
                    DirectedAction::MoveWest
                } else {
                    DirectedAction::None
                };

                return (action, ExecutionStatus::InProgress);
            } else {
                return (DirectedAction::None, ExecutionStatus::Wait);
            }
        }

        execute_move_to(world, player_index, self.wait_pos, execution_state)
    }

    fn name(&self) -> String {
        "OpenDoorCoop".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::PassThroughDoorWithPlate as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.target_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        let other_player_index = 1 - player_index;
        let other_player_pos = if other_player_index < world.players.len() {
            Some(world.players[other_player_index].position)
        } else {
            None
        };

        let players_can_reach_each_other = if let Some(other_pos) = other_player_pos {
            world.find_path(player.position, other_pos).is_some()
        } else {
            false
        };

        let has_unexplored = !player.unexplored_frontier.is_empty();
        if players_can_reach_each_other && has_unexplored {
            return actions;
        }

        for color in [Color::Red, Color::Green, Color::Blue] {
            if let (Some(door_positions), Some(plate_positions)) = (
                world.doors.get_positions(color),
                world.pressure_plates.get_positions(color),
            ) {
                for door_pos in door_positions {
                    for plate_pos in plate_positions {
                        let adjacent_pairs = [
                            (
                                Position::new(door_pos.x - 1, door_pos.y),
                                Position::new(door_pos.x + 1, door_pos.y),
                            ),
                            (
                                Position::new(door_pos.x + 1, door_pos.y),
                                Position::new(door_pos.x - 1, door_pos.y),
                            ),
                            (
                                Position::new(door_pos.x, door_pos.y - 1),
                                Position::new(door_pos.x, door_pos.y + 1),
                            ),
                            (
                                Position::new(door_pos.x, door_pos.y + 1),
                                Position::new(door_pos.x, door_pos.y - 1),
                            ),
                        ];

                        for (wait_pos, target_pos) in adjacent_pairs {
                            if player.position == target_pos || player.position == wait_pos {
                                continue;
                            }

                            if wait_pos.x < 0
                                || wait_pos.x >= world.map.width
                                || wait_pos.y < 0
                                || wait_pos.y >= world.map.height
                            {
                                continue;
                            }
                            if target_pos.x < 0
                                || target_pos.x >= world.map.width
                                || target_pos.y < 0
                                || target_pos.y >= world.map.height
                            {
                                continue;
                            }

                            if !world.is_walkable(&wait_pos, None) {
                                continue;
                            }
                            if world.find_path(player.position, wait_pos).is_none() {
                                continue;
                            }

                            if world.find_path(player.position, target_pos).is_some() {
                                continue;
                            }

                            let action = PassThroughDoorWithPlateAction {
                                door_color: color,
                                door_pos: *door_pos,
                                wait_pos,
                                target_pos,
                                plate_pos: *plate_pos,
                            };
                            if action.precondition(world, player_index) {
                                actions.push(Box::new(action) as Box<dyn RLActionTrait>);
                            }
                        }
                    }
                }
            }
        }

        actions
    }
}
