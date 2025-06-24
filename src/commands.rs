use std::collections::HashMap;

use crate::game::Game;

pub struct CommandRegistry {
    commands: HashMap<String, fn(&mut Game, &Command) -> Option<()>>,
    pub previous_commands: Vec<Command>,
}


pub struct Command {
    string: String
}


pub struct CommandArg<'me> {
    text: &'me str,
}


impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            previous_commands: vec![],
        }
    }


    pub fn register(&mut self, base: &str, command: fn(&mut Game, &Command) -> Option<()>) {
        self.commands.insert(base.to_string(), command);
    }


    pub fn find(&self, command: &str) -> Option<fn(&mut Game, &Command) -> Option<()>> {
        self.commands.get(command).copied()
    }
}


impl Command {
    pub fn parse(str: String) -> Self {
        Self { string: str }
    }


    pub fn command(&self) -> &str { 
        self.string.split_whitespace().next().unwrap()
    }


    pub fn arg<'me>(&'me self, index: usize) -> Option<CommandArg<'me>> {
        let command = self.string.split_whitespace().skip(index+1).next()?;
        Some(CommandArg {
            text: command,
        })
    }


    pub fn as_str(&self) -> &str { &self.string }
}


impl<'me> CommandArg<'me> {
    pub fn as_f64(&self) -> Option<f64> {
        self.text.parse().ok()
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.text.parse().ok()
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.text.parse().ok()
    }

    pub fn as_u32(&self) -> Option<u32> {
        self.text.parse().ok()
    }

    pub fn as_i64(&self) -> Option<i64> {
        self.text.parse().ok()
    }

    pub fn as_i32(&self) -> Option<i32> {
        self.text.parse().ok()
    }

    pub fn as_str(&self) -> &'me str {
        self.text
    }
}
