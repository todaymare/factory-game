use glam::{Vec3, Vec4};

pub const COLOUR_PASS : Vec3 = Vec3::new(0.5, 0.8, 0.5);
pub const COLOUR_DENY : Vec3 = Vec3::new(0.8, 0.5, 0.5);

pub const MSAA_SAMPLE_COUNT : u32 = 4;
pub const VOXEL_TEXTURE_ATLAS_TILE_SIZE : u32 = 32;
pub const VOXEL_TEXTURE_ATLAS_TILE_CAP : u32 = 256;

pub const UI_CROSSAIR_SIZE        : f32  = 8.0;
pub const UI_CROSSAIR_COLOUR      : Vec4 = Vec4::ONE;
pub const UI_HOTBAR_UNSELECTED_BG : Vec4 = Vec4::new(0.2, 0.2, 0.2, 1.0);
pub const UI_HOTBAR_SELECTED_BG   : Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);
pub const UI_SLOT_SIZE            : f32  = 60.0;
pub const UI_ITEM_OFFSET          : f32  = UI_SLOT_SIZE * 0.05;
pub const UI_ITEM_SIZE            : f32  = UI_SLOT_SIZE * 0.9;
pub const UI_ITEM_AMOUNT_SCALE    : f32  = 0.5;
pub const UI_SLOT_PADDING         : f32  = 16.0;

pub const REGION_SIZE    : usize = 32;
pub const REGION_SIZE_P3 : usize = REGION_SIZE*REGION_SIZE*REGION_SIZE;

pub const CHUNK_SIZE     : usize = 32;
pub const CHUNK_SIZE_PAD : usize = CHUNK_SIZE+2;
pub const CHUNK_SIZE_P3  : usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;
pub const CHUNK_SIZE_I32 : i32 = CHUNK_SIZE as i32;

pub const MOUSE_SENSITIVITY : f32 = 0.0016;

pub const PLAYER_REACH : f32 = 5.0;
pub const PLAYER_SPEED : f32 = 10.0;
pub const PLAYER_PULL_DISTANCE : f32 = 3.5;
pub const PLAYER_INTERACT_DELAY : f32 = 0.125;
pub const PLAYER_HOTBAR_SIZE : usize = 5;
pub const PLAYER_ROW_SIZE : usize = 6;
pub const PLAYER_INVENTORY_SIZE : usize = PLAYER_ROW_SIZE * PLAYER_HOTBAR_SIZE;

pub const RENDER_DISTANCE : i32 = 4;

pub const DROPPED_ITEM_SCALE : f32 = 0.5;

pub const TICKS_PER_SECOND : u32 = 60;
pub const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


pub const QUAD_VERTICES : &[i32] = &[
     0,   0,  0,
     1,   0,  0,
     1,   0,  1,
     0,   0,  1,
];


pub const QUAD_INDICES : &[u32] = &[0, 1, 2, 2, 3, 0];

