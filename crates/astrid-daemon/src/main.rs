//! Companion daemon binary entry point.
//!
//! The runnable binary is declared by the `astrid` package and delegates to
//! the shared `astrid_daemon::run()` library function.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    astrid_daemon::run().await
}
