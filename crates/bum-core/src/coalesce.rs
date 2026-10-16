//! Gộp các thay đổi dồn dập thành tối đa một sự kiện cho UI mỗi `every`.
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Change {
    pub prompt_keys: Vec<String>,
    pub session_ids: Vec<String>,
}

pub struct Coalescer {
    every: Duration,
    last: Option<Instant>,
    pending: Change,
}

fn push_all(into: &mut Vec<String>, items: &[String]) {
    for i in items {
        if !into.contains(i) {
            into.push(i.clone());
        }
    }
}

impl Coalescer {
    pub fn new(every: Duration) -> Self {
        Self { every, last: None, pending: Change::default() }
    }

    pub fn add(&mut self, prompt_keys: &[String], session_ids: &[String]) {
        push_all(&mut self.pending.prompt_keys, prompt_keys);
        push_all(&mut self.pending.session_ids, session_ids);
    }

    /// Lấy thay đổi đang chờ nếu đã đủ khoảng cách với lần phát trước.
    pub fn take(&mut self, now: Instant) -> Option<Change> {
        if self.pending == Change::default() {
            return None;
        }
        if self.last.is_some_and(|l| now.duration_since(l) < self.every) {
            return None;
        }
        self.last = Some(now);
        Some(std::mem::take(&mut self.pending))
    }
}
