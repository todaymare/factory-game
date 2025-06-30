use glam::{Vec3, Vec4};


pub const REGION_SIZE : u32 = 32;

pub const COLOUR_PASS : Vec3 = Vec3::new(0.5, 0.8, 0.5);
pub const COLOUR_DENY : Vec3 = Vec3::new(0.8, 0.5, 0.5);


pub const UI_CROSSAIR_SIZE        : f32  = 8.0;
pub const UI_CROSSAIR_COLOUR      : Vec4 = Vec4::ONE;
pub const UI_HOTBAR_UNSELECTED_BG : Vec4 = Vec4::new(0.2, 0.2, 0.2, 1.0);
pub const UI_HOTBAR_SELECTED_BG   : Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);
pub const UI_SLOT_SIZE            : f32  = 60.0;
pub const UI_ITEM_OFFSET          : f32  = UI_SLOT_SIZE * 0.05;
pub const UI_ITEM_SIZE            : f32  = UI_SLOT_SIZE * 0.9;
pub const UI_ITEM_AMOUNT_SCALE    : f32  = 0.5;
pub const UI_SLOT_PADDING         : f32  = 16.0;


pub const CHUNK_SIZE     : usize = 32;
pub const CHUNK_SIZE_P3  : usize = CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE;
pub const CHUNK_SIZE_I32 : i32 = CHUNK_SIZE as i32;


