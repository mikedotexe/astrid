use astrid_source_study::owner_transaction::OwnerTransaction;
use std::{sync::mpsc, time::Duration};

#[test]
fn nested_transaction_holds_cross_thread_lock_until_the_last_guard() {
    let temp = tempfile::tempdir().unwrap();
    let outer = OwnerTransaction::acquire(temp.path()).unwrap();
    let inner = OwnerTransaction::acquire(temp.path()).unwrap();
    let path = temp.path().to_owned();
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _guard = OwnerTransaction::acquire(&path).unwrap();
        tx.send(()).unwrap();
    });
    drop(outer);
    assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
    drop(inner);
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    thread.join().unwrap();
    // The thread-local weak entry does not retain the OS lock.
    OwnerTransaction::acquire(temp.path()).unwrap();
}
