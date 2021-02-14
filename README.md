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
wasm-rs-async-executor = "0.5.1"
```

`wasm-rs-async-executor` is expected to work on stable Rust, 1.49.0 and higher up. It *may* also
work on earlier versions. This hasn't been tested yet.

## Notes

Please note that this library hasn't received much analysis in terms of safety
and soundness. Some of the caveats related to that might never be resolved
completely. This is an ongoing development and the maintainer is aware of
potential pitfalls. Any productive reports of unsafeties or unsoundness are
welcomed (whether they can be resolved or simply walled with `unsafe` for end-user
to note).

## FAQ

**Q:** Why not just use [wasm-bindgen-futures](https://rustwasm.github.io/wasm-bindgen/api/wasm_bindgen_futures/)?

**A:** *(short)* In many cases, `wasm-bindgen-futures` is just fine

**A:** *(longer)* There are some subtle differences in the way `wasm-rs-async-executor` exposes its functionality. The
core idea behind it is to expose a reasonable amount of **explicit** controls over the its operations. You need
to run the executor explicitly, you can block on certain futures (again, explicitly -- which gives you a limited
ability to do scoped futures). It does come with minor trade-offs. For example, if you want to use async APIs from
your host environment, you can't simply `await` on then, as the executor won't yield to the browser until told to do
so (using `yield_async(future).await`). Ultimately, if this amount of control is beneficial for your case, then perhaps
this executor is waranted. It is also important to note that **currently** `wasm-rs-async-executor` **does not** support
WebAssembly multi-threading in any way. `wasm-bindgen-futures` does, if the standard library is built with support for it.
It is planned to support this, but hasn't been high priority so far due to
[current state of things](https://github.com/rust-lang/rust/issues/77839) in Rust.


## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT) at your option.
