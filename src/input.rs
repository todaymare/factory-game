use std::collections::HashSet;

use glam::{IVec2, Vec2};
use glfw::{Key, MouseButton};

#[derive(Debug, Default)]
pub struct InputManager {
    down_keys: HashSet<Key>,
    down_buttons: HashSet<MouseButton>,
    just_pressed_key: Vec<Key>,
    just_pressed_button: Vec<MouseButton>,

    mouse_pos: Vec2,
    delta_mouse_pos: Vec2,
}


impl InputManager {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::NAN,
            ..Default::default()
        }
    }


    pub fn update(&mut self) {
        self.just_pressed_key.clear();
        self.just_pressed_button.clear();
        self.delta_mouse_pos = Vec2::ZERO;
    }


    pub fn set_pressed_key(&mut self, key: Key) {
        self.down_keys.insert(key);
        self.just_pressed_key.push(key);
    }


    pub fn set_unpressed_key(&mut self, key: Key) {
        self.down_keys.remove(&key);
    }


    pub fn set_pressed_button(&mut self, button: MouseButton) {
        self.down_buttons.insert(button);
        self.just_pressed_button.push(button);
    }


    pub fn set_unpressed_button(&mut self, button: MouseButton) {
        self.down_buttons.remove(&button);
    }


    pub fn move_cursor(&mut self, new_pos: Vec2) {
        if !self.mouse_pos.is_nan() {
            self.delta_mouse_pos = new_pos - self.mouse_pos;
        }
        self.mouse_pos = new_pos;
    }


    pub fn is_key_pressed(&self, key: Key) -> bool {
        self.down_keys.contains(&key)
    }


    pub fn is_key_just_pressed(&self, key: Key) -> bool {
        self.just_pressed_key.iter().find(|x| **x == key).is_some()
    }


    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.down_buttons.contains(&button)
    }


    pub fn is_button_just_pressed(&self, button: MouseButton) -> bool {
        self.just_pressed_button.iter().find(|x| **x == button).is_some()
    }


    pub fn mouse_position(&self) -> Vec2 { self.mouse_pos }
    pub fn mouse_delta(&self) -> Vec2 { self.delta_mouse_pos }
}




