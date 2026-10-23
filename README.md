# winprint

[![crates.io](https://img.shields.io/crates/v/winprint.svg)](https://crates.io/crates/winprint)
[![Released API docs](https://docs.rs/winprint/badge.svg)](https://docs.rs/winprint)
[![BSD 3 Clause licensed](https://img.shields.io/badge/license-BSD%203%20Clause-blue)](./LICENSE.md)

A crate for printing to a Windows printer device using Windows API.

## About Safety
This crate interfaces with several Windows API functions, necessitating the use of unsafe code blocks. Nevertheless, this crate is designed to be safe and sound to use. If you find any case that will break safety or soundness, please report it as a bug.

## Development
### Testing on Windows
Virtual devices are automatically created and used for testing, so simple `cargo test` will be okey.

For efficiency, [`cargo hack`](https://github.com/taiki-e/cargo-hack) and [`cargo nextest`](https://nexte.st/) may be used. The recommended command is:

```bash
cargo hack nextest run --all-targets --each-feature --skip default,test-utils --no-fail-fast
```

Some fixtures are required for tests, and they are shipped in the repo. If you want to learn how they are generated, see `utils/gen-visual-regression-data/`.

### Testing on Linux Host
Since this crate is designed for Windows, it cannot be directly tested on a Linux host. However, a script is provided to run the test suite inside a Windows 11 VM on a Linux host.

See [`utils/test-in-windows-vm/README.md`](./utils/test-in-windows-vm/README.md) for details.

## License
Licensed under [BSD 3 Clause](./LICENSE.md)

### Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, shall be licensed as BSD 3 Clause, without any additional terms or conditions.
