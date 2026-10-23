#!/usr/bin/env bash
# Usage: exec-in-container.sh WORKSPACE_NAME COMMAND [ARG ...] < checkout.tar
set -euo pipefail

workspace_name=${1:?Expected workspace name}
shift
[[ "${workspace_name}" =~ ^workspace_[a-z2-7]{20}$ ]] || {
  echo "Error: invalid workspace name." >&2
  exit 2
}
[[ $# -gt 0 ]] || { echo "Error: expected a command." >&2; exit 2; }
ssh_config=/run/windows-ssh-config

# Serialize runs for the same checkout, while allowing other checkouts.
exec 9>"/run/${workspace_name}.lock"
flock 9

archive="$(mktemp)"
trap 'rm -f "${archive}"' EXIT
cat > "${archive}"
echo "Uploading to D:/${workspace_name}"
scp -F "${ssh_config}" "${archive}" "default:D:/${workspace_name}.tar"

powershell_command="$(cat <<'POWERSHELL'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
Set-StrictMode -Version Latest

$workspace = 'D:/__WORKSPACE_NAME__'
$archive = 'D:/__WORKSPACE_NAME__.tar'
New-Item -ItemType Directory -Path $workspace -Force | Out-Null
try {
    # Remove deleted source files, but retain incremental build artifacts.
    Get-ChildItem -LiteralPath $workspace -Force |
        Where-Object Name -NE 'target' |
        Remove-Item -Recurse -Force
    tar.exe -xf $archive -C $workspace
} finally {
    Remove-Item -LiteralPath $archive -Force
}

# Cargo and kache read their global configuration written while provisioning.
$env:RUST_BACKTRACE = '1'
$env:WORKSPACE_PATH = $workspace
Set-Location -LiteralPath $workspace
POWERSHELL
)"
powershell_command="${powershell_command//__WORKSPACE_NAME__/${workspace_name}}"

# Preserve argument boundaries and literal PowerShell metacharacters. Base64
# sidesteps PowerShell quoting, which also treats U+2018-U+201B as quotes.
# For pipelines or scripts, callers can explicitly use pwsh -Command '...'.
command_arguments="$(
  # shellcheck disable=SC2016 # This is PowerShell source, not a shell variable.
  printf '$command = @(\n'
  for argument in "$@"; do
    printf "  [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('%s'))\n" \
      "$(printf '%s' "${argument}" | base64 --wrap=0)"
  done
  printf ')\n'
)"
powershell_command+=$'\n'"${command_arguments}"$'\n'
powershell_command+="$(cat <<'POWERSHELL'
$commandName = $command[0]
$commandArguments = @($command | Select-Object -Skip 1)

# Preserve native exit codes; terminating PowerShell errors exit with 1.
$PSNativeCommandUseErrorActionPreference = $false
$global:LASTEXITCODE = 0
& $commandName @commandArguments
if (-not $?) {
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    exit 1
}
exit $LASTEXITCODE
POWERSHELL
)"
encoded_command="$(
  printf '%s' "${powershell_command}" |
    iconv --from-code=UTF-8 --to-code=UTF-16LE |
    base64 --wrap=0
)"
ssh -F "${ssh_config}" default \
  "& 'C:/Program Files/PowerShell/7/pwsh.exe' -NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand ${encoded_command}; exit \$LASTEXITCODE"
