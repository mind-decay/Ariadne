use std::collections::HashMap;

pub struct Counter {
    pub value: u32,
}

pub enum Tick {
    Up,
    Down,
}

pub trait Step {
    fn step(&mut self, t: Tick);
}

pub type Bag = HashMap<String, u32>;

impl Counter {
    pub fn new(start: u32) -> Self {
        Self { value: start }
    }

    pub fn increment(&mut self) -> u32 {
        self.value += 1;
        self.value
    }
}

pub fn fresh() -> Counter {
    Counter::new(0)
}

mod inner {
    pub fn ping() -> u32 {
        42
    }
}
