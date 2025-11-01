use std::collections::HashSet;

use crate::swoq_interface::Inventory;
use crate::types::Position;

#[allow(dead_code)]
pub trait Player {
    fn position(&self) -> Position;
    fn health(&self) -> i32;
    fn inventory(&self) -> Inventory;
    fn has_sword(&self) -> bool;
    fn current_goal(&self) -> Option<crate::goals::Goal>;
    fn previous_goal(&self) -> Option<crate::goals::Goal>;
    fn current_destination(&self) -> Option<Position>;
    fn current_path(&self) -> Option<Vec<Position>>;
    fn unexplored_frontier(&self) -> &HashSet<Position>;

    fn set_current_destination(&mut self, dest: Option<Position>);
    fn set_current_path(&mut self, path: Option<Vec<Position>>);
    fn set_current_goal(&mut self, goal: Option<crate::goals::Goal>);
    fn set_previous_goal(&mut self, goal: Option<crate::goals::Goal>);

    fn sorted_unexplored(&self) -> Vec<Position>;
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub position: Position,
    pub health: i32,
    pub inventory: Inventory,
    pub has_sword: bool,
    pub is_active: bool,
    // Planning state to avoid oscillation
    pub current_goal: Option<crate::goals::Goal>,
    pub previous_goal: Option<crate::goals::Goal>,
    pub current_destination: Option<Position>,
    pub current_path: Option<Vec<Position>>,
    pub unexplored_frontier: HashSet<Position>,
}

impl PlayerState {
    pub fn new(pos: Position) -> Self {
        Self {
            position: pos,
            health: 10,
            inventory: Inventory::None,
            has_sword: false,
            is_active: true,
            current_goal: None,
            previous_goal: None,
            current_destination: None,
            current_path: None,
            unexplored_frontier: HashSet::new(),
        }
    }

    pub fn clear(&mut self) {
        self.inventory = Inventory::None;
        self.has_sword = false;
        self.is_active = true;
        self.current_goal = None;
        self.previous_goal = None;
        self.current_destination = None;
        self.current_path = None;
        self.unexplored_frontier.clear();
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sorted_unexplored(&self) -> Vec<Position> {
        let mut frontier: Vec<Position> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.position.distance(pos));
        frontier
    }
}

impl Player for PlayerState {
    fn position(&self) -> Position {
        self.position
    }

    fn health(&self) -> i32 {
        self.health
    }

    fn inventory(&self) -> Inventory {
        self.inventory
    }

    fn has_sword(&self) -> bool {
        self.has_sword
    }

    fn current_goal(&self) -> Option<crate::goals::Goal> {
        self.current_goal.clone()
    }

    fn previous_goal(&self) -> Option<crate::goals::Goal> {
        self.previous_goal.clone()
    }

    fn current_destination(&self) -> Option<Position> {
        self.current_destination
    }

    fn current_path(&self) -> Option<Vec<Position>> {
        self.current_path.clone()
    }

    fn unexplored_frontier(&self) -> &HashSet<Position> {
        &self.unexplored_frontier
    }

    fn set_current_destination(&mut self, dest: Option<Position>) {
        self.current_destination = dest;
    }

    fn set_current_path(&mut self, path: Option<Vec<Position>>) {
        self.current_path = path;
    }

    fn set_current_goal(&mut self, goal: Option<crate::goals::Goal>) {
        self.current_goal = goal;
    }

    fn set_previous_goal(&mut self, goal: Option<crate::goals::Goal>) {
        self.previous_goal = goal;
    }

    fn sorted_unexplored(&self) -> Vec<Position> {
        self.sorted_unexplored()
    }
}

impl Player for &mut PlayerState {
    fn position(&self) -> Position {
        self.position
    }

    fn health(&self) -> i32 {
        self.health
    }

    fn inventory(&self) -> Inventory {
        self.inventory
    }

    fn has_sword(&self) -> bool {
        self.has_sword
    }

    fn current_goal(&self) -> Option<crate::goals::Goal> {
        self.current_goal.clone()
    }

    fn previous_goal(&self) -> Option<crate::goals::Goal> {
        self.previous_goal.clone()
    }

    fn current_destination(&self) -> Option<Position> {
        self.current_destination
    }

    fn current_path(&self) -> Option<Vec<Position>> {
        self.current_path.clone()
    }

    fn unexplored_frontier(&self) -> &HashSet<Position> {
        &self.unexplored_frontier
    }

    fn set_current_destination(&mut self, dest: Option<Position>) {
        self.current_destination = dest;
    }

    fn set_current_path(&mut self, path: Option<Vec<Position>>) {
        self.current_path = path;
    }

    fn set_current_goal(&mut self, goal: Option<crate::goals::Goal>) {
        self.current_goal = goal;
    }

    fn set_previous_goal(&mut self, goal: Option<crate::goals::Goal>) {
        self.previous_goal = goal;
    }

    fn sorted_unexplored(&self) -> Vec<Position> {
        PlayerState::sorted_unexplored(self)
    }
}
