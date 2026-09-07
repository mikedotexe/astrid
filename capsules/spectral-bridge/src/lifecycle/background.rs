//! Process-owned work that must finish after producer admission closes.
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Default)]
pub(super) struct WorkGroup {
    state: Mutex<(usize, bool)>,
}

#[derive(Debug)]
pub(super) struct WorkGuard {
    group: Arc<WorkGroup>,
    completed: bool,
}

impl WorkGuard {
    pub(super) fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for WorkGuard {
    fn drop(&mut self) {
        let mut state = self.group.state.lock().expect("background work lock");
        state.0 = state.0.saturating_sub(1);
        state.1 |= !self.completed;
    }
}

impl WorkGroup {
    fn spawn<F>(self: &Arc<Self>, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let guard = self.begin();
        tokio::spawn(async move {
            let result = future.await;
            guard.complete();
            result
        })
    }

    pub(super) fn begin(self: &Arc<Self>) -> WorkGuard {
        let mut state = self.state.lock().expect("background work lock");
        state.0 = state
            .0
            .checked_add(1)
            .expect("background work counter overflow");
        WorkGuard {
            group: self.clone(),
            completed: false,
        }
    }

    pub(super) async fn wait(&self) -> anyhow::Result<()> {
        loop {
            let (active, failed) = *self.state.lock().expect("background work lock");
            if active == 0 {
                anyhow::ensure!(
                    !failed,
                    "admitted work ended without confirmed completion; drain unconfirmed"
                );
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
}

fn group() -> &'static Arc<WorkGroup> {
    static GROUP: OnceLock<Arc<WorkGroup>> = OnceLock::new();
    GROUP.get_or_init(|| Arc::new(WorkGroup::default()))
}

pub fn spawn_background<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    group().spawn(future)
}

pub fn spawn_blocking_background<F, T>(work: F) -> tokio::task::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let guard = group().begin();
    tokio::task::spawn_blocking(move || {
        let result = work();
        guard.complete();
        result
    })
}

/// Call only after every root producer has stopped, so zero is stable.
pub async fn wait_background() -> anyhow::Result<()> {
    group().wait().await
}

pub fn spawn_background_thread<F, T>(
    name: String,
    work: F,
) -> std::io::Result<std::thread::JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let guard = group().begin();
    std::thread::Builder::new().name(name).spawn(move || {
        let result = work();
        guard.complete();
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawned_worker_owns_guard_until_completion() {
        let group = Arc::new(WorkGroup::default());
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = group.spawn(async { rx.await.unwrap() });
        assert_eq!(*group.state.lock().unwrap(), (1, false));
        tx.send(7).unwrap();
        assert_eq!(task.await.unwrap(), 7);
        group.wait().await.unwrap();
    }

    #[tokio::test]
    async fn aborted_spawn_is_not_successful_drain() {
        let group = Arc::new(WorkGroup::default());
        let task = group.spawn(std::future::pending::<()>());
        task.abort();
        assert!(task.await.is_err());
        assert!(group.wait().await.is_err());
    }

    #[tokio::test]
    async fn nested_work_prevents_early_drain() {
        let group = Arc::new(WorkGroup::default());
        let mut parent = group.begin();
        let mut child = group.begin();
        parent.completed = true;
        drop(parent);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(40), group.wait())
                .await
                .is_err()
        );
        child.completed = true;
        drop(child);
        group.wait().await.unwrap();
    }

    #[tokio::test]
    async fn dropped_worker_cannot_be_acknowledged() {
        let group = Arc::new(WorkGroup::default());
        drop(group.begin());
        assert!(group.wait().await.is_err());
    }
}
