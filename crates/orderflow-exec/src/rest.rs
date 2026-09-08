//! REST weight priority. Never hits the network in stage 6.

use serde::{Deserialize, Serialize};

/// Hard-coded: risk > cancel > flatten > open > query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestPriority {
    Risk = 0,
    Cancel = 1,
    Flatten = 2,
    Open = 3,
    Query = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestOp {
    pub priority: RestPriority,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct RestQueue {
    items: Vec<RestOp>,
}

impl RestQueue {
    pub fn push(&mut self, priority: RestPriority, name: impl Into<String>) {
        self.items.push(RestOp {
            priority,
            name: name.into(),
        });
    }

    /// Lowest priority number first. Does not send HTTP.
    pub fn pop(&mut self) -> Option<RestOp> {
        if self.items.is_empty() {
            return None;
        }
        let i = self
            .items
            .iter()
            .enumerate()
            .min_by_key(|(_, op)| op.priority as u8)
            .map(|(i, _)| i)?;
        Some(self.items.remove(i))
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
