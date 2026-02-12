//! Simple connection pool for managing multiple transport connections.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;

use crate::Transport;

/// A round-robin pool of transports.
///
/// Messages are dispatched to the next available transport in a circular
/// fashion.  This is useful for load-balancing across multiple server
/// connections or for multiplexing channels.
pub struct Pool<T: Transport> {
    transports: Vec<T>,
    cursor: AtomicUsize,
}

impl<T: Transport> Pool<T> {
    /// Create an empty pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            transports: Vec::new(),
            cursor: AtomicUsize::new(0),
        }
    }

    /// Add a transport to the pool.
    pub fn push(&mut self, transport: T) {
        self.transports.push(transport);
    }

    /// Number of transports in the pool.
    #[must_use]
    pub fn len(&self) -> usize {
        self.transports.len()
    }

    /// Whether the pool is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transports.is_empty()
    }

    /// Send data via the next transport in round-robin order.
    pub async fn send(&self, data: Bytes) -> crate::Result<()> {
        if self.transports.is_empty() {
            return Err(crate::TransportError::NotConnected);
        }
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed) % self.transports.len();
        self.transports[idx].send(data).await
    }

    /// Receive data from the transport at the given index.
    pub async fn recv_from(&self, index: usize) -> crate::Result<Bytes> {
        self.transports
            .get(index)
            .ok_or(crate::TransportError::NotConnected)?
            .recv()
            .await
    }

    /// Return a list of peer addresses for all connected transports.
    #[must_use]
    pub fn peers(&self) -> Vec<Option<SocketAddr>> {
        self.transports.iter().map(|t| t.peer_addr()).collect()
    }

    /// Close all transports in the pool.
    pub async fn close_all(&mut self) -> crate::Result<()> {
        for t in &mut self.transports {
            t.close().await?;
        }
        self.transports.clear();
        Ok(())
    }

    /// Remove and return all transports from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.cursor.store(0, Ordering::Relaxed);
        std::mem::take(&mut self.transports)
    }
}

impl<T: Transport> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}
