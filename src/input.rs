use std::collections::HashSet;

use glam::Vec2;
use glfw::{Key, MouseButton};

#[derive(Debug, Default)]
pub struct InputManager {
    down_keys: HashSet<Key>,
    down_buttons: HashSet<MouseButton>,
    just_pressed_key: Vec<Key>,
    just_pressed_button: Vec<MouseButton>,
    current_chars: Vec<char>,

    mouse_pos: Vec2,
    scroll_dt: Vec2,
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
        self.current_chars.clear();
        self.delta_mouse_pos = Vec2::ZERO;
        self.scroll_dt = Vec2::ZERO;
    }


    pub fn new_char(&mut self, ch: char) {
        self.current_chars.push(ch);
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


    pub fn scroll(&mut self, sdt: Vec2) {
        self.scroll_dt = sdt;
    }


    pub fn scroll_delta(&self) -> Vec2 { self.scroll_dt }


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


    pub fn is_super_pressed(&self) -> bool {
        self.is_key_pressed(Key::LeftSuper) || self.is_key_pressed(Key::RightSuper)
    }


    pub fn is_alt_pressed(&self) -> bool {
        self.is_key_pressed(Key::LeftAlt) || self.is_key_pressed(Key::RightAlt)
    }


    pub fn should_paste(&self) -> bool {
        #[cfg(target_os="macos")]
        {
            (self.is_key_pressed(Key::LeftSuper) || self.is_key_pressed(Key::RightSuper))
            && self.is_key_pressed(Key::V)
        }
    }

    pub fn should_paste_now(&self) -> bool {
        #[cfg(target_os="macos")]
        {
            (self.is_key_pressed(Key::LeftSuper) || self.is_key_pressed(Key::RightSuper))
            && self.is_key_just_pressed(Key::V)
        }
    }


    pub fn should_delete_word(&self) -> bool {
        #[cfg(target_os="macos")]
        {
            (self.is_key_pressed(Key::LeftAlt) || self.is_key_pressed(Key::RightAlt))
            && self.is_key_pressed(Key::Backspace)
        }
    }

    pub fn should_delete_word_now(&self) -> bool {
        #[cfg(target_os="macos")]
        {
            (self.is_key_pressed(Key::LeftAlt) || self.is_key_pressed(Key::RightAlt))
            && self.is_key_just_pressed(Key::Backspace)
        }
    }


    pub fn should_delete_line(&self) -> bool {
        #[cfg(target_os="macos")]
        {
            (self.is_key_pressed(Key::LeftSuper) || self.is_key_pressed(Key::RightSuper))
            && self.is_key_just_pressed(Key::Backspace)
        }
    }


    pub fn mouse_position(&self) -> Vec2 { self.mouse_pos }
    pub fn mouse_delta(&self) -> Vec2 { self.delta_mouse_pos }
    pub fn current_chars(&self) -> &[char] {
        &self.current_chars
    }
}




