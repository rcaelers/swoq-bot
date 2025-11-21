mod boulder_tracker;
mod cbs;
mod composite_observer;
mod default_observer;
mod game_observer;
mod item_tracker;
mod pathfinding;
pub mod swoq;
mod types;
mod visualizing_observer;

pub use boulder_tracker::BoulderTracker;
pub use cbs::{Agent, CBS};
pub use composite_observer::CompositeObserver;
pub use default_observer::DefaultObserver;
pub use game_observer::GameObserver;
pub use item_tracker::{ColoredItemTracker, ItemTracker};
pub use pathfinding::AStar;
pub use swoq::GameConnection;
pub use types::{Bounds, Color, Position};
pub use visualizing_observer::VisualizingObserver;

use crate::swoq_interface::DirectedAction;

// ============================================================================
// Helper functions
// ============================================================================

pub fn path_to_action(current: Position, path: &[Position]) -> Option<DirectedAction> {
    if path.len() < 2 {
        return None;
    }
    let next = path[1];

    if next.y < current.y {
        Some(DirectedAction::MoveNorth)
    } else if next.y > current.y {
        Some(DirectedAction::MoveSouth)
    } else if next.x > current.x {
        Some(DirectedAction::MoveEast)
    } else if next.x < current.x {
        Some(DirectedAction::MoveWest)
    } else {
        None
    }
}

pub fn use_direction(from: Position, to: Position) -> DirectedAction {
    if to.y < from.y {
        DirectedAction::UseNorth
    } else if to.y > from.y {
        DirectedAction::UseSouth
    } else if to.x > from.x {
        DirectedAction::UseEast
    } else {
        DirectedAction::UseWest
    }
}
