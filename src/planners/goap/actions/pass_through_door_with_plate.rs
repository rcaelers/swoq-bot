use crate::infra::{Color, Position};
use crate::planners::goap::game_state::PlanningState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PassThroughDoorWithPlateAction {
    pub door_color: Color,
    pub door_pos: Position,
    pub target_pos: Position,
    pub plate_pos: Position,
}

impl PassThroughDoorWithPlateAction {
    fn check_execute_precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];
        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));
        let door_not_open = !world.is_door_open(self.door_color);
        door_exists
            && plate_exists
            && door_not_open
            && world.find_path(player.position, self.target_pos).is_some()
    }
}

impl GOAPActionTrait for PassThroughDoorWithPlateAction {
    fn precondition(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> bool {
        let player = &world.players[player_index];
        let door_exists = world
            .doors
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.door_pos));
        let plate_exists = world
            .pressure_plates
            .get_positions(self.door_color)
            .is_some_and(|positions| positions.contains(&self.plate_pos));
        let door_not_open = !world.is_door_open(self.door_color);
        door_exists
            && plate_exists
            && door_not_open
            && world.find_path(player.position, self.target_pos).is_some()
    }

    fn effect_end(&self, world: &mut WorldState, _state: &mut PlanningState, player_index: usize) {
        world.players[player_index].position = self.target_pos;
    }

    fn prepare(&mut self, _world: &mut WorldState, _player_index: usize) -> Option<Position> {
        Some(self.target_pos)
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        // Check precondition before executing
        if !self.check_execute_precondition(world, player_index) {
            return (DirectedAction::None, ExecutionStatus::Wait);
        }

        execute_move_to(world, player_index, self.target_pos, execution_state)
    }

    fn cost(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> f32 {
        10.0 + world
            .path_distance(world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, world: &WorldState, _state: &PlanningState, player_index: usize) -> u32 {
        // Distance to door + distance through door to target + coordination overhead
        let to_door = world
            .path_distance(world.players[player_index].position, self.door_pos)
            .unwrap_or(1000) as u32;
        let through_door = self.door_pos.distance(&self.target_pos) as u32;
        to_door + through_door + 3 // +3 ticks for coordination
    }

    fn name(&self) -> String {
        "PassThroughDoorWithPlate".to_string()
    }

    fn is_pass_through_door_with_plate(&self) -> Option<(Color, Position, Position)> {
        Some((self.door_color, self.door_pos, self.plate_pos))
    }

    fn generate(
        world: &WorldState,
        state: &PlanningState,
        player_index: usize,
    ) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &world;

        for color in [
            crate::infra::Color::Red,
            crate::infra::Color::Green,
            crate::infra::Color::Blue,
        ] {
            if let (Some(door_positions), Some(plate_positions)) =
                (world.doors.get_positions(color), world.pressure_plates.get_positions(color))
            {
                for door_pos in door_positions {
                    for plate_pos in plate_positions {
                        // Try to find positions adjacent to the door as targets
                        let adjacent_positions = [
                            Position::new(door_pos.x - 1, door_pos.y),
                            Position::new(door_pos.x + 1, door_pos.y),
                            Position::new(door_pos.x, door_pos.y - 1),
                            Position::new(door_pos.x, door_pos.y + 1),
                        ];

                        for target_pos in adjacent_positions {
                            // Simple validation that target is within map bounds
                            if target_pos.x >= 0
                                && target_pos.x < world.map.width
                                && target_pos.y >= 0
                                && target_pos.y < world.map.height
                            {
                                let action = PassThroughDoorWithPlateAction {
                                    door_color: color,
                                    door_pos: *door_pos,
                                    target_pos,
                                    plate_pos: *plate_pos,
                                };
                                if action.precondition(world, state, player_index) {
                                    actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                                }
                            }
                        }
                    }
                }
            }
        }

        actions
    }
}
