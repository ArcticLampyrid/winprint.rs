## Project Overview
`winprint` is a crate for printing to a Windows printer device using Windows API.

## Programming Style
- Prefer pipeline style over imperative style when possible.
- Pay attention to data flow and state management.
- Prefer immutable data structures when possible.
- Follow SSOT (Single Source of Truth) principle. Avoid duplicating data and logic across the codebase.
- Write modular code. Break down large functions into smaller, reusable functions. But avoid creating too many small functions that make the code hard to follow.
- Keep related data and functions together. Prefer to use AoS (Array of Structures) over SoA (Structure of Arrays) when it improves code clarity.
- Keep performance in mind, but do not sacrifice code readability for minor performance gains.

## Commands
- `cargo test` - Run the test suite on a Windows machine.
- `cargo hack` - Run tests with multiple features in one command.
- `./utils/test-in-windows-vm/run.sh` - Run the test suite inside a Windows 11 VM on a Linux host.

## Shell Behavior
- In PowerShell on Windows, paths are generally insensitive to forward and backslashes; prefer forward slashes when needed, or use path normalization to get the backslash version.
- Prefer pwsh over cmd.exe and legacy PowerShell for scripting and automation tasks on Windows.
- Do not use aliases in scripts. Use full command names instead.

## Commit Guidelines
- Use stacked commits for large changes. Each commit should represent a single logical change.
- Follow conventional commit style for commit messages.
- Rebase and squash commits when necessary to keep the commit history clean and meaningful.
