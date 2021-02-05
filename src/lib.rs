//! # Async Executor for WebAssembly
//!
//! There are a number of async task executors available, however, most of them rely on primitives
//! that might not be available or optimal for WebAssembly deployment at the time.
//!
//! This crate currently provides one [**single-threaded** async executor](`single_threaded`)
pub mod single_threaded;
