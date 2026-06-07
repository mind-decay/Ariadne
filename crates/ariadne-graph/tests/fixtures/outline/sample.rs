//! Sample module for outline tests.
//! Second line of module doc.

use std::collections::HashMap;
use std::fmt;

/// A documented public function.
/// It has a multi-line body.
pub fn greet(name: &str) -> String {
    let mut out = String::new();
    out.push_str("hello ");
    out.push_str(name);
    out
}

fn helper(x: u32) -> u32 {
    x * 2
}

/// A public struct holding state.
pub struct Counter {
    value: u32,
}

impl Counter {
    /// Construct a fresh counter.
    pub fn new() -> Self {
        Counter { value: 0 }
    }

    /// Increment and return the new value.
    pub fn bump(&mut self) -> u32 {
        self.value += 1;
        self.value
    }
}
