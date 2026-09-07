//! Track accepted evidence writes without changing bounded admission or retry policy.
use super::background::{WorkGroup, WorkGuard};
use std::ops::Deref;
use std::sync::{Arc, OnceLock, mpsc};

fn group() -> &'static Arc<WorkGroup> {
    static WRITES: OnceLock<Arc<WorkGroup>> = OnceLock::new();
    WRITES.get_or_init(|| Arc::new(WorkGroup::default()))
}

#[derive(Debug)]
pub struct Sender<T>(mpsc::SyncSender<Item<T>>);
pub struct Receiver<T>(mpsc::Receiver<Item<T>>);

#[derive(Debug)]
pub struct Item<T> {
    value: T,
    guard: WorkGuard,
}

impl<T> Deref for Item<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> Item<T> {
    pub fn complete(self) {
        self.guard.complete();
    }
}

impl<T> Sender<T> {
    pub fn try_send(&self, value: T) -> Result<(), mpsc::TrySendError<T>> {
        let item = Item {
            value,
            guard: group().begin(),
        };
        self.0.try_send(item).map_err(|error| match error {
            mpsc::TrySendError::Full(item) => {
                item.guard.complete();
                mpsc::TrySendError::Full(item.value)
            },
            mpsc::TrySendError::Disconnected(item) => {
                item.guard.complete();
                mpsc::TrySendError::Disconnected(item.value)
            },
        })
    }
}

impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<Item<T>, mpsc::RecvError> {
        self.0.recv()
    }
}

pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::sync_channel(capacity);
    (Sender(tx), Receiver(rx))
}

/// Telemetry and all other evidence producers must be stopped first.
pub async fn wait() -> anyhow::Result<()> {
    group().wait().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queued_item_remains_pending_until_written_not_just_received() {
        let group = Arc::new(WorkGroup::default());
        let (tx, rx) = mpsc::sync_channel(1);
        tx.send(Item {
            value: 12,
            guard: group.begin(),
        })
        .unwrap();
        let item = rx.recv().unwrap();
        assert_eq!(*item, 12);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(40), group.wait())
                .await
                .is_err()
        );
        item.complete();
        group.wait().await.unwrap();
    }
}
