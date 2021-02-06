<!-- next-header -->

## [Unreleased] - ReleaseDate

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
[Unreleased]: https://github.com/wasm-rs/async-executor/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/wasm-rs/async-executor/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wasm-rs/async-executor/compare/v0.1.0...v0.2.0
