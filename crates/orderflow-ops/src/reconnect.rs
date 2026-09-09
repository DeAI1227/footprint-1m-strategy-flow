//! Bounded reconnect backoff. Never tight-loop the public WS.

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub initial_ms: u64,
    pub max_ms: u64,
    pub current_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_ms: 500,
            max_ms: 30_000,
            current_ms: 500,
        }
    }
}

impl ReconnectPolicy {
    pub fn next_delay_ms(&mut self) -> u64 {
        let d = self.current_ms;
        self.current_ms = (self.current_ms.saturating_mul(2)).min(self.max_ms);
        d
    }

    pub fn reset(&mut self) {
        self.current_ms = self.initial_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_then_caps() {
        let mut p = ReconnectPolicy::default();
        assert_eq!(p.next_delay_ms(), 500);
        assert_eq!(p.next_delay_ms(), 1000);
        for _ in 0..20 {
            let d = p.next_delay_ms();
            assert!(d <= 30_000);
        }
        assert_eq!(p.current_ms, 30_000);
    }
}
