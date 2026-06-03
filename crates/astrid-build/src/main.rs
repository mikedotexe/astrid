//! Companion `astrid-build` entry point.
//!
//! The runnable binary is declared by the `astrid` package so release builds
//! co-install all companion binaries from one owner.

fn main() -> anyhow::Result<()> {
    astrid_build::run()
}
