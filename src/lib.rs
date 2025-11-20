pub mod infra;
pub mod planners;
pub mod state;
pub mod ui;

// Re-export commonly used types for convenience
pub use infra::{Position, CBS, AStar};
pub use state::Map;

// Re-export proto interface
pub mod swoq_interface {
    include!(concat!(env!("OUT_DIR"), "/swoq.interface.rs"));
}
