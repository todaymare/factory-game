use glam::{Vec3, Vec4};

use crate::directions::Direction;

// helper


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
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z),
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y, pos.z),
            ],
            Direction::Right => [
                Vec3::new(pos.x, pos.y, pos.z),
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
                Vec3::new(pos.x, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
            ],
            Direction::Down => [
                Vec3::new(pos.x, pos.y, pos.z),
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
                Vec3::new(pos.x + 1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
            ],
            Direction::Up => [
                Vec3::new(pos.x,   pos.y+1.0, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z),
                Vec3::new(pos.x    , pos.y+1.0, pos.z),
            ],
            Direction::Back => [
                Vec3::new(pos.x, pos.y, pos.z),
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
            ],
            Direction::Forward => [
                Vec3::new(pos.x + 1.0, pos.y, pos.z+1.0),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x, pos.y, pos.z+1.0),
            ],
        };

        Self {
            corners,
            color,
            direction,
        }
    }
}
