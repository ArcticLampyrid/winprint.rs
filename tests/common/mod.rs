//! Test helpers shared across integration tests.

#![cfg(all(windows, feature = "test-utils"))]
pub mod loader;
pub mod print;
pub mod visual;
