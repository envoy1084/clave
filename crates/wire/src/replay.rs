use std::collections::{HashMap, VecDeque};

use crate::{Error, Header, IdentityPublic};

/// Rejects duplicate ids per signer, messages from other epochs, and timestamps outside a window.
///
/// Keeps at most `capacity` remembered ids (oldest evicted); messages older than the window are rejected outright,
/// so an evicted id cannot be replayed once it falls outside the window.
pub struct ReplayGuard {
    max_skew: u64,
    capacity: usize,
    seen: HashMap<(IdentityPublic, [u8; 16]), u64>,
    order: VecDeque<(IdentityPublic, [u8; 16])>,
}

impl ReplayGuard {
    /// A guard accepting timestamps within `max_skew` seconds of the receiver's clock.
    #[must_use]
    pub fn new(max_skew: u64, capacity: usize) -> Self {
        Self {
            max_skew,
            capacity: capacity.max(1),
            seen: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Accepts the message once, or rejects it.
    ///
    /// # Errors
    /// [`Error::WrongEpoch`], [`Error::Stale`] or [`Error::Replayed`].
    pub fn check(
        &mut self,
        signer: &IdentityPublic,
        header: &Header,
        epoch: u32,
        now: u64,
    ) -> Result<(), Error> {
        if header.epoch != epoch {
            return Err(Error::WrongEpoch {
                expected: epoch,
                actual: header.epoch,
            });
        }
        if header.created_at.abs_diff(now) > self.max_skew {
            return Err(Error::Stale);
        }
        let key = (*signer, header.id);
        if self.seen.contains_key(&key) {
            return Err(Error::Replayed);
        }
        if self.order.len() == self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.seen.remove(&oldest);
        }
        self.seen.insert(key, header.created_at);
        self.order.push_back(key);
        Ok(())
    }
}
