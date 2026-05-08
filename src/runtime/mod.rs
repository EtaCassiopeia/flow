#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio_util::sync::CancellationToken;

use crate::msg::FetchId;

/// Tracks in-flight fetches and their cancellation tokens.
#[derive(Default)]
pub struct Registry {
    next: AtomicU64,
    tokens: HashMap<FetchId, CancellationToken>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issue(&mut self) -> (FetchId, CancellationToken) {
        let id = FetchId(self.next.fetch_add(1, Ordering::Relaxed));
        let token = CancellationToken::new();
        self.tokens.insert(id, token.clone());
        (id, token)
    }

    pub fn cancel(&mut self, id: FetchId) {
        if let Some(t) = self.tokens.remove(&id) {
            t.cancel();
        }
    }

    pub fn complete(&mut self, id: FetchId) -> bool {
        self.tokens.remove(&id).is_some()
    }

    pub fn cancel_all(&mut self) {
        for (_, t) in self.tokens.drain() {
            t.cancel();
        }
    }

    pub fn in_flight(&self) -> usize {
        self.tokens.len()
    }
}
