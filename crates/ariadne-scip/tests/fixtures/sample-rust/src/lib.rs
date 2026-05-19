//! Tiny fixture crate used to generate `tests/fixtures/sample.scip` for the
//! tier-05 round-trip test. Kept deliberately minimal so the resulting SCIP
//! index is small enough to commit (a few KB).

pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn tick(&mut self) -> i64 {
        self.value += 1;
        self.value
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}
