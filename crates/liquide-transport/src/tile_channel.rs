//! Minimal tile batch transport channel for local loopback testing.
//!
//! This is a **synchronous** in-process channel specialized for
//! [`TileBatch`] transmission. It is intended for integration tests and
//! local development smoke testing, not for production network transport.

use std::sync::mpsc;
use liquide_encoder::tile::TileBatch;

/// Sender half of a tile batch channel.
#[derive(Clone)]
pub struct TileSender {
    tx: mpsc::Sender<TileBatch>,
}

impl TileSender {
    /// Send a tile batch through the channel.
    ///
    /// Returns `Err` if the receiver has been dropped.
    pub fn send(&self, batch: TileBatch) -> Result<(), mpsc::SendError<TileBatch>> {
        self.tx.send(batch)
    }

    /// Try to send without blocking.
    pub fn try_send(&self, batch: TileBatch) -> Result<(), mpsc::TrySendError<TileBatch>> {
        // mpsc::Sender doesn't have try_send in std, only send which blocks
        // if the channel is full. For unbounded channels this is fine.
        self.tx.send(batch).map_err(|e| mpsc::TrySendError::Disconnected(e.0))
    }
}

/// Receiver half of a tile batch channel.
pub struct TileReceiver {
    rx: mpsc::Receiver<TileBatch>,
}

impl TileReceiver {
    /// Receive the next tile batch, blocking if necessary.
    ///
    /// Returns `None` if the sender has been dropped and no batches remain.
    pub fn recv(&self) -> Option<TileBatch> {
        self.rx.recv().ok()
    }

    /// Try to receive without blocking.
    ///
    /// Returns `None` if no batch is currently available or the sender
    /// has been dropped.
    pub fn try_recv(&self) -> Option<TileBatch> {
        self.rx.try_recv().ok()
    }

    /// Iterator over remaining batches in the channel.
    pub fn try_iter(&self) -> impl Iterator<Item = TileBatch> + '_ {
        self.rx.try_iter()
    }
}

/// Create a new unbounded tile batch channel.
///
/// Returns `(sender, receiver)` pair. The sender can be cloned; the
/// receiver is single-consumer.
#[must_use]
pub fn tile_channel() -> (TileSender, TileReceiver) {
    let (tx, rx) = mpsc::channel();
    (TileSender { tx }, TileReceiver { rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_channel_send_recv() {
        let (tx, rx) = tile_channel();
        let batch = TileBatch::new(42);
        tx.send(batch).unwrap();
        let received = rx.recv().unwrap();
        assert_eq!(received.sequence, 42);
    }

    #[test]
    fn tile_channel_try_recv_empty() {
        let (_tx, rx) = tile_channel();
        assert!(rx.try_recv().is_none());
    }

    #[test]
    fn tile_channel_sender_drop_disconnects() {
        let (tx, rx) = tile_channel();
        drop(tx);
        assert!(rx.recv().is_none());
    }

    #[test]
    fn tile_channel_multiple_batches() {
        let (tx, rx) = tile_channel();
        for i in 0..5 {
            tx.send(TileBatch::new(i)).unwrap();
        }
        let batches: Vec<_> = rx.try_iter().collect();
        assert_eq!(batches.len(), 5);
        for (i, batch) in batches.iter().enumerate() {
            assert_eq!(batch.sequence, i as u64);
        }
    }
}
