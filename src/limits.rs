use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

pub struct TransferLimits {
    active: Arc<Mutex<ActiveTransfers>>,
    total_limit: usize,
    client_limit: usize,
}

#[derive(Default)]
struct ActiveTransfers {
    total: usize,
    clients: HashMap<IpAddr, usize>,
}

pub struct TransferPermit {
    active: Arc<Mutex<ActiveTransfers>>,
    client: IpAddr,
}

impl TransferLimits {
    pub fn new(total_limit: usize, client_limit: usize) -> Self {
        Self {
            active: Arc::new(Mutex::new(ActiveTransfers::default())),
            total_limit,
            client_limit,
        }
    }

    pub fn try_acquire(&self, client: IpAddr) -> Option<TransferPermit> {
        let mut active = self.active.lock().expect("transfer limiter mutex poisoned");
        if active.total >= self.total_limit {
            return None;
        }
        let count = active.clients.entry(client).or_default();
        if *count >= self.client_limit {
            return None;
        }
        *count += 1;
        active.total += 1;
        Some(TransferPermit {
            active: self.active.clone(),
            client,
        })
    }
}

impl Drop for TransferPermit {
    fn drop(&mut self) {
        let mut active = self.active.lock().expect("transfer limiter mutex poisoned");
        let count = active
            .clients
            .get_mut(&self.client)
            .expect("active transfer has a client");
        *count -= 1;
        if *count == 0 {
            // Retain addresses only while they have active transfers.
            active.clients.remove(&self.client);
        }
        active.total -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_clients_and_total_and_reuses_slots() {
        let limiter = TransferLimits::new(2, 1);
        let a = "192.0.2.1".parse().unwrap();
        let b = "192.0.2.2".parse().unwrap();
        let c = "192.0.2.3".parse().unwrap();
        let first = limiter.try_acquire(a).unwrap();
        assert!(limiter.try_acquire(a).is_none());
        let second = limiter.try_acquire(b).unwrap();
        assert!(limiter.try_acquire(c).is_none());
        drop(first);
        assert!(limiter.try_acquire(c).is_some());
        drop(second);
        assert!(limiter.try_acquire(a).is_some());
    }
}
