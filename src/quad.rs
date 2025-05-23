use glam::{IVec3, Vec3, Vec4};

// helper
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
    ///! normal data is packed in the shader
    pub fn get_normal(&self) -> i32 {
        match self {
            Direction::Left => 0i32,
            Direction::Right => 1i32,
            Direction::Down => 2i32,
            Direction::Up => 3i32,
            Direction::Back => 4i32,
            Direction::Forward => 5i32,
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

#[derive(Debug)]
///! plane data with 4 vertices
pub struct Quad {
    pub color: Vec4,
    pub direction: Direction,
    pub corners: [Vec3; 4],
}

impl Quad {
    // the input position is assumed to be a voxel's (0,0,0) pos
    // therefore right / up / forward are offset by 1
    #[inline]
    pub fn from_direction(direction: Direction, pos: Vec3, color: Vec4) -> Self {
        let corners = match direction {
            Direction::Left => [
                Vec3::new(pos.x+1.0, pos.y, pos.z),
                Vec3::new(pos.x+1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z),
            ],
            Direction::Right => [
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Down => [
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Up => [
                Vec3::new(pos.x    , pos.y+1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z + 1.0),
                Vec3::new(pos.x,   pos.y+1.0, pos.z + 1.0),
            ],
            Direction::Back => [
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Forward => [
                Vec3::new(pos.x, pos.y, pos.z+1.0),
                Vec3::new(pos.x, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z+1.0),
            ],
        };

        Self {
            corners,
            color,
            direction,
        }
    }
}
