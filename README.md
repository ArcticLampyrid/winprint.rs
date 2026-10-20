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

Also there are visual regression tests for PDF, XPS, and Image files.
Each prints the matching origin file from
`test_data/visual_regression/data.*` through a virtual Microsoft PWG
Raster printer, decodes the PWG bitstream, and SSIM-compares each page
against the corresponding `data.tiff`.

Day-to-day development never needs to regenerate these fixtures — the repo
ships them and `cargo test` consumes them as-is. If you do need to rebuild
them (e.g. after tweaking the source drawing or bumping a baseline renderer),
run `python utils/gen_visual_regression_data/run.py`; see the README in that
directory for tool prerequisites and determinism guarantees.

For testing with multiple features in one command, [`cargo hack`](https://github.com/taiki-e/cargo-hack) may be used for convenience.

### Testing on Linux Host
Since this crate is designed for Windows, it cannot be directly tested on a Linux host. However, a script is provided to run the test suite inside a Windows 11 VM on a Linux host.

**Prerequisites:** Podman/Docker with BuildKit, KVM support (`/dev/kvm`), OpenSSH clients (`ssh`, `scp`), and sufficient disk space & memory for the VM.

```bash
./utils/test-in-windows-vm/run.sh              # Start the VM if needed, then run tests
./utils/test-in-windows-vm/run.sh --auto-stop  # Same, then stop the VM unless another run uses it

./utils/test-in-windows-vm/run.sh start        # Create the container if needed and boot the VM
./utils/test-in-windows-vm/run.sh test         # Start if needed, sync the workspace, and then run tests
./utils/test-in-windows-vm/run.sh exec echo 0  # Start, sync and run custom commands (single, not shell)
./utils/test-in-windows-vm/run.sh stop         # Shut down the VM and remove the container
./utils/test-in-windows-vm/run.sh prune        # Remove the VM and guest/build state
./utils/test-in-windows-vm/run.sh prune --all  # Also remove Vagrant boxes and image
./utils/test-in-windows-vm/run.sh ssh          # Interactive PowerShell 7 session
./utils/test-in-windows-vm/run.sh scp ./file.txt windows:D:/file.txt
```

## License
Licensed under [BSD 3 Clause](./LICENSE.md)

### Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, shall be licensed as BSD 3 Clause, without any additional terms or conditions.
