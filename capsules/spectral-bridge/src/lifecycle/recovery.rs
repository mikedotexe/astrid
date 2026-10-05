//! Drain-only recovery. Retrying persistence must not repeat admitted work.
use tokio::sync::watch;

/// The final acknowledgement binds both independently persisted state families.
pub struct DrainCheckpoint {
    pub conversation: super::FrozenWrite,
    pub feedback: super::FrozenWrite,
}

impl DrainCheckpoint {
    pub fn verify(&mut self) -> std::io::Result<()> {
        self.feedback.publish()?;
        self.conversation.publish()
    }
}

/// One attempt per operator signal after failure; no busy retry or new admission.
pub async fn retry_on_signal<T>(
    stage: &'static str,
    retry: &mut watch::Receiver<()>,
    mut operation: impl FnMut() -> anyhow::Result<T>,
) -> T {
    loop {
        retry.borrow_and_update();
        match operation() {
            Ok(value) => return value,
            Err(error) => tracing::error!(
                %error, stage,
                "drain persistence blocked; state retained; a new drain/exit signal retries only this operation"
            ),
        }
        if retry.changed().await.is_err() {
            // Losing the operator channel is not permission to drop the state.
            std::future::pending::<()>().await;
        }
    }
}

/// A failed producer cannot be replayed or called drained. Keep its owner alive.
pub async fn retain_on_failure<T>(stage: &'static str, result: anyhow::Result<T>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, stage, "drain unconfirmed; retaining state without replay or forced exit");
            std::future::pending().await
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    #[tokio::test]
    async fn failed_publication_retains_owner_and_never_reexecutes_work() {
        struct Owned(Arc<AtomicUsize>);
        impl Drop for Owned {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let attempts = Arc::new(AtomicUsize::new(0));
        let executed = Arc::new(AtomicUsize::new(0));
        let (tx, mut rx) = watch::channel(());
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let owned = Owned(dropped.clone());
        let counted = attempts.clone();
        let work = executed.clone();
        let task = tokio::spawn(async move {
            work.fetch_add(1, Ordering::SeqCst);
            retry_on_signal("fixture checkpoint", &mut rx, || {
                if counted.fetch_add(1, Ordering::SeqCst) < 2 {
                    anyhow::bail!("disk sync failed");
                }
                Ok(())
            })
            .await;
            drop(owned);
            done_tx.send(()).unwrap();
        });
        for expected in 1..=2 {
            while attempts.load(Ordering::SeqCst) < expected {
                tokio::task::yield_now().await;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            assert_eq!(attempts.load(Ordering::SeqCst), expected);
            assert_eq!(dropped.load(Ordering::SeqCst), 0);
            assert_eq!(executed.load(Ordering::SeqCst), 1);
            tx.send_replace(());
        }
        done_rx.await.unwrap();
        task.await.unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn closed_retry_channel_does_not_acknowledge_failure() {
        let (tx, mut rx) = watch::channel(());
        drop(tx);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(20),
                retry_on_signal::<()>("fixture", &mut rx, || anyhow::bail!("failed"))
            )
            .await
            .is_err()
        );
    }
}
