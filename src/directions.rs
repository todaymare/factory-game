use glam::{IVec3, Vec3};

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
            CardinalDirection::South => IVec3::new(0, 0, 1),
            CardinalDirection::North => IVec3::new(0, 0, -1),
            CardinalDirection::East => IVec3::new(1, 0, 0),
            CardinalDirection::West => IVec3::new(-1, 0, 0),
        }
    }


    pub fn from_index(i: u8) -> CardinalDirection {
        match i {
            3 => CardinalDirection::North,
            2 => CardinalDirection::West,
            1 => CardinalDirection::South,
            0 => CardinalDirection::East,
            _ => unreachable!(),
        }
    }

    pub fn to_index(self) -> u8 {
        match self {
            CardinalDirection::North => 3,
            CardinalDirection::West => 2,
            CardinalDirection::South => 1,
            CardinalDirection::East => 0,
        }
    }


    pub fn next_n(self, n: u8) -> Self {
        let index = self.to_index();
        let index = index + n;
        let index = index % 4;
        Self::from_index(index)
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
    pub const NORMALS : &[Vec3] = &[
        Vec3::new( 1.0,  0.0,  0.0),
        Vec3::new( 0.0,  1.0,  0.0),
        Vec3::new( 0.0,  0.0,  1.0),
        Vec3::new(-1.0,  0.0,  0.0),
        Vec3::new( 0.0, -1.0,  0.0),
        Vec3::new( 0.0,  0.0, -1.0),
        
    ];

    
    pub fn from_normal(normal: u8) -> Direction {
        match normal {
            0 => Direction::Left,
            1 => Direction::Up,
            2 => Direction::Forward,
            3 => Direction::Right,
            4 => Direction::Down,
            5 => Direction::Back,
            _ => unreachable!(),
        }
    }


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
