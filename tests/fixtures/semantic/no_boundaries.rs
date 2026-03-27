// Semantic fixture: Rust file with no framework patterns
// Expected boundaries:
//   Producers: 0
//   Consumers: 0
//   Total: 0

pub fn compute(input: &[u8]) -> Vec<u8> {
    input.to_vec()
}

pub struct Config {
    pub timeout: u64,
    pub retries: u32,
}

impl Config {
    pub fn new() -> Self {
        Config {
            timeout: 30,
            retries: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute() {
        assert_eq!(compute(b"hello"), b"hello");
    }
}
