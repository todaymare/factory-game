use glam::IVec3;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CardinalDirection {
    North,
    South,
    East,
    West,
}


impl CardinalDirection {
    pub fn as_ivec3(self) -> IVec3 {
        match self {
            CardinalDirection::North => IVec3::new(0, 0, 1),
            CardinalDirection::South => IVec3::new(0, 0, -1),
            CardinalDirection::East => IVec3::new(1, 0, 0),
            CardinalDirection::West => IVec3::new(-1, 0, 0),
        }
    }
}



#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Left,
    Right,
    Down,
    Up,
    Back,
    Forward,
}


impl Direction {
    pub fn get_opposite(self) -> Self {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::Back => Direction::Forward,
            Direction::Forward => Direction::Back,
        }
    }
}
