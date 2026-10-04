use std::sync::atomic::{AtomicUsize, Ordering};

use super::models::Receipt;

#[derive(Default)]
pub struct BillingService {
    accepted: AtomicUsize,
}

impl BillingService {
    pub fn accept(&self) -> Receipt {
        Receipt {
            id: self.accepted.fetch_add(1, Ordering::SeqCst) + 1,
            state: "accepted",
        }
    }

    pub fn count(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}
