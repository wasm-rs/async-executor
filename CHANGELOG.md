<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.9.0] - 2021-02-21

### Changed

- `single_threaded::block_on` returns `R` instead of `Option<R>` now

## [0.8.1] - 2021-02-20

### Added

- Continuous integration with `wasm32-wasi` target

## [0.8.0] - 2021-02-19

### Changed

- `single_threaded::spawn` now returns `TaskHandle<T>` that allows joining
  until task completion and retrieval of task's result
- Tasks are no longer required to return `()`

## [0.7.0] - 2021-02-17

### Added

- `TypeId` information on the task is now available through `Task.type_info().type_id()`

### Changed

- `single_threaded::Token` type is no longer public
- `single_threaded::tasks()` has been renamed to `tasks_count()`
- `single_threaded::queued_tasks()` has been renamed to `tasks_count()`
- `single_threaded::tokens()` has been replaced with `tasks()` and returns `Vec<Task>` now;
   it's no longer under `debug` feature gate
- `single_threaded::queued_tokens()` has been replaced with `queued_tasks()` and returns `Vec<Task>` now;
   it's no longer under `debug` feature gate
- `single_threaded::task_type()` has been replaced with `Task.type_info().type_name()`

## [0.6.1] - 2021-02-14

### Fixed

- `single_threaded::yield_animation_frame` and `single_threaded::yield_until_idle` futures might
  panic at times

## [0.6.0] - 2021-02-13

### Added

- `single_threaded::yield_timeout`, `single_threaded::yield_async` and `single_threaded::yield_animation_frame` API
  to orchestrate different types of yielding to the environment

### Removed

- `single_threaded::run_cooperatively` is removed in favour of
  `single_threaded::yield_timeout` and `single_threaded::yield_animation_frame`

## [0.5.1] - 2021-02-08

### Changed

- `single_threaded::run_cooperatively` is no longer marked `unsafe`
  (but this is not a guarantee just yet)

## [0.5.0] - 2021-02-08

### Added

- A way to run the executor cooperatively with the host's JavaScript environment
  (`single_threaded::run_cooperatively`)

## [0.4.1] - 2021-02-06

The snapshot of 0.4.0's code was published incorrectly, thus yanked. 0.4.1 replaces it.

## [0.4.0] - 2021-02-06

### Fixed

- Type debugging information was removed too early
- Incorrect future pinning

## [0.3.2] - 2021-02-06

### Added

- Debugging capabilities in a form of `single_threaded::task_name`, `single_threaded::tokens` and
  `single_threaded::queued_tokens`

## [0.3.1] - 2021-02-06

### Fixed

- Executor can starve if all tasks went pending

## [0.3.0] - 2021-02-06

### Added

- `single_threaded::block_on` API that allows to block on non-static-lifetime futures

## [0.2.0] - 2021-02-06

### Added

- `single_threaded::tasks` and `single_threaded::queued_tasks` API for executor summary
- `single_threaded::evict_all` API to permanently remove all current tasks 

### Fixed

- single-threaded executor's token counter can panic on exhaustion

## [0.1.0] - 2021-02-05

Initial release

<!-- next-url -->
[Unreleased]: https://github.com/wasm-rs/async-executor/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/wasm-rs/async-executor/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/wasm-rs/async-executor/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/wasm-rs/async-executor/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/wasm-rs/async-executor/compare/v0.6.1...v0.7.0
[0.6.1]: https://github.com/wasm-rs/async-executor/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/wasm-rs/async-executor/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/wasm-rs/async-executor/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/wasm-rs/async-executor/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/wasm-rs/async-executor/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/wasm-rs/async-executor/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/wasm-rs/async-executor/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/wasm-rs/async-executor/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/wasm-rs/async-executor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wasm-rs/async-executor/compare/v0.1.0...v0.2.0
