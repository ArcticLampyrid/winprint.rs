//! Test helpers shared across integration tests. Not every test uses every helper.

#![cfg(all(windows, feature = "test-utils"))]
#![allow(dead_code)]
pub mod color;
pub mod loader;
pub mod print;
pub mod visual;
