# Test in Windows VM
This directory contains a script to run a Windows 11 VM (via Vagrant and KVM) inside a Linux container (Podman/Docker with BuildKit). It automates setup, workspace syncing, test execution, custom commands, and VM lifecycle management.

## Prerequisites
Podman/Docker with BuildKit, KVM support (`/dev/kvm`), OpenSSH clients (`ssh`, `scp`), and sufficient disk space & memory for the VM.

## Software in Image
- Windows 11 Pro (x64)
- PowerShell 7
- Visual Studio Build Tools 2022
- LLVM
- kache
- Rust toolchain (stable)
- cargo-hack: cargo subcommand to provide various options useful for testing and continuous integration.
- cargo-nextest

## Usage
### Basic
```bash
./utils/test-in-windows-vm/run.sh              # Start if needed, sync the workspace, and then run all tests
./utils/test-in-windows-vm/run.sh test         # Alias for the above command
./utils/test-in-windows-vm/run.sh --auto-stop  # Same, then stop the VM unless another run uses it
```

### Custom Commands
For a single command (not a shell), you can run:
```bash
# Start, sync and run custom commands
./utils/test-in-windows-vm/run.sh exec cargo nextest run --no-fail-fast
```

For an script, you may run:
```js
const utf16leToBase64 = (str) => {
  let charCodes = [];
  for (let i = 0; i < str.length; i++) {
    let code = str.charCodeAt(i);
    charCodes.push(code & 0xff);
    charCodes.push((code >> 8) & 0xff);
  }
  let binaryString = String.fromCharCode.apply(null, charCodes);
  return btoa(binaryString);
}
const script = `
echo 0
`;
text(await tools.bash({command:`./utils/test-in-windows-vm/run.sh exec pwsh -NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand "${utf16leToBase64(script)}"`,"timeout":15}));
```

### Lifecycle
```bash
./utils/test-in-windows-vm/run.sh start        # Create the container if needed and boot the VM
./utils/test-in-windows-vm/run.sh stop         # Shut down the VM and remove the container
./utils/test-in-windows-vm/run.sh prune        # Remove the VM and guest/build state
./utils/test-in-windows-vm/run.sh prune --all  # Also remove Vagrant boxes and image
```

### Interaction
You can use `ssh` and `scp` to interact with it.

IMPORTANT: This won't sync the workspace.

```bash
./utils/test-in-windows-vm/run.sh ssh          # PowerShell 7 session
./utils/test-in-windows-vm/run.sh scp ./file.txt windows:D:/file.txt
```
