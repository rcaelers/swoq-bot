#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: &Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn neighbors(&self) -> [Position; 4] {
        [
            Position::new(self.x, self.y - 1), // North
            Position::new(self.x + 1, self.y), // East
            Position::new(self.x, self.y + 1), // South
            Position::new(self.x - 1, self.y), // West
        ]
    }

    pub fn is_adjacent(&self, other: &Position) -> bool {
        self.distance(other) == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl Bounds {
    #[allow(dead_code)]
    pub fn new(min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    pub fn from_center_and_range(center: Position, range: i32) -> Self {
        Self {
            min_x: center.x - range,
            max_x: center.x + range,
            min_y: center.y - range,
            max_y: center.y + range,
        }
    }

    pub fn contains(&self, pos: &Position) -> bool {
        pos.x >= self.min_x && pos.x <= self.max_x && pos.y >= self.min_y && pos.y <= self.max_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Red,
    Green,
    Blue,
}
