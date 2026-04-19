# winprint

[![crates.io](https://img.shields.io/crates/v/winprint.svg)](https://crates.io/crates/winprint)
[![Released API docs](https://docs.rs/winprint/badge.svg)](https://docs.rs/winprint)
[![BSD 3 Clause licensed](https://img.shields.io/badge/license-BSD%203%20Clause-blue)](./LICENSE.md)

A crate for printing to a Windows printer device using Windows API.

## About Safety
This crate interfaces with several Windows API functions, necessitating the use of unsafe code blocks. Nevertheless, this crate is designed to be safe and sound to use. If you find any case that will break safety or soundness, please report it as a bug.

## Development
### Testing on Windows
Just run `cargo test` on a Windows machine, null printer will be automatically created and used for testing. The tests cover basic printer operations like getting printer information, writing to the printer, etc.

For testing with multiple features in one command, [`cargo hack`](https://github.com/taiki-e/cargo-hack) may be used for convenience.

### Testing on Linux Host
Since this crate is designed for Windows, it cannot be directly tested on a Linux host. However, a script is provided to run the test suite inside a Windows 11 VM on a Linux host.

**Prerequisites:** Docker with BuildKit, KVM support (`/dev/kvm`), and sufficient disk space & memory for the VM.

```bash
./utils/test-in-windows-vm/run.sh
```

The script will spins up a Windows VM and then run the test suite inside it. On first run it initializes the VM (very slowly). Subsequent runs reuse the cached VM state (relatively fast).

To clear the cached VM state, run with `--destroy` flag:

```bash
./utils/test-in-windows-vm/run.sh --destroy
```

## License
Licensed under [BSD 3 Clause](./LICENSE.md)

### Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, shall be licensed as BSD 3 Clause, without any additional terms or conditions.
