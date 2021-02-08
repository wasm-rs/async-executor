# Async Executor for WebAssembly
[![Crate](https://img.shields.io/crates/v/wasm-rs-async-executor.svg)](https://crates.io/crates/wasm-rs-async-executor)
[![API](https://docs.rs/wasm-rs-async-executor/badge.svg)](https://docs.rs/wasm-rs-async-executor)
[![Chat](https://img.shields.io/discord/807386653852565545.svg?logo=discord)](https://discord.gg/qbcbjHWjaD)

There are a number of async task executors available in Rust's ecosystem.
However, most (if not all?) of them rely on primitives that might not be
available or optimal for WebAssembly deployment at the time.

## Usage

Include this dependency in your `Cargo.toml`:

```toml
[dependencies]
wasm-rs-async-executor = "0.5.0"
```

## Notes

Please note that this library hasn't received much analysis in terms of safety
and soundness. Some of the caveats related to that might never be resolved
completely. This is an ongoing development and the maintainer is aware of
potential pitfalls. Any productive reports of unsafeties or unsoundness are
welcomed (whether they can be resolved or simply walled with `unsafe` for end-user
to note).

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT) at your option.
