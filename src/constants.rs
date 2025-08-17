use glam::{Vec2, Vec4};

pub const COLOUR_WHITE: Vec4 = Vec4::new(1.0, 1.0, 1.0, 1.0);
pub const COLOUR_PASS : Vec4 = Vec4::new(0.2, 0.8, 0.2, 1.0);
pub const COLOUR_WARN : Vec4 = Vec4::new(0.8, 0.8, 0.2, 1.0);
pub const COLOUR_DENY : Vec4 = Vec4::new(0.8, 0.2, 0.2, 1.0);
pub const COLOUR_GREY : Vec4 = Vec4::new(0.2, 0.2, 0.2, 1.0);
pub const COLOUR_DARK_GREY : Vec4 = Vec4::new(0.1, 0.1, 0.1, 1.0);
pub const COLOUR_SCREEN_DIM : Vec4 = Vec4::new(0.1, 0.1, 0.1, 0.6);
pub const COLOUR_PLAYER_ACTIVE_HOTBAR : Vec4 = Vec4::new(0.4, 0.6, 0.4, 1.0);

pub const COLOUR_ADDITIVE_HIGHLIGHT: Vec4 = Vec4::splat(0.4);

pub const MSAA_SAMPLE_COUNT : u32 = 4;
pub const VOXEL_TEXTURE_ATLAS_TILE_SIZE : u32 = 32;
pub const VOXEL_TEXTURE_ATLAS_TILE_CAP : u32 = 256;

pub const UI_CROSSAIR_SIZE        : f32  = 8.0;
pub const UI_CROSSAIR_COLOUR      : Vec4 = Vec4::ONE;
pub const UI_HOTBAR_UNSELECTED_BG : Vec4 = Vec4::new(0.2, 0.2, 0.2, 1.0);
pub const UI_HOTBAR_SELECTED_BG   : Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);
pub const UI_SLOT_SIZE            : f32  = 60.0;
pub const UI_HOVER_ACTION_OFFSET  : Vec2 = Vec2::new(30.0, 0.0); 
pub const UI_ITEM_OFFSET          : f32  = UI_SLOT_SIZE * 0.05;
pub const UI_ITEM_SIZE            : f32  = UI_SLOT_SIZE * 0.9;
pub const UI_ITEM_AMOUNT_SCALE    : f32  = 0.5;
pub const UI_SLOT_PADDING         : f32  = 16.0;
pub const UI_DELTA_Z              : f32  = 0.0001;
pub const UI_Z_MAX                : f32  = 1.0;
pub const UI_Z_MIN                : f32  = 0.0;

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

pub const RENDER_DISTANCE : i32 = 16;
pub const LOAD_DISTANCE : i32 = 4;

pub const FONT_SIZE : u32 = 48;

pub const DROPPED_ITEM_SCALE : f32 = 0.5;

pub const TICKS_PER_SECOND : u32 = 60;
pub const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


pub const COAL_ENERGY_PER_UNIT : u32 = 200;
pub const FURNACE_COST_PER_SMELT : u32 = 50;


pub const QUAD_VERTICES : &[i32] = &[
     1,   0,  1, 0,
     1,   0,  0, 1,
     0,   0,  0, 2,
     0,   0,  0, 3,
     0,   0,  1, 4,
     1,   0,  1, 5,

];
