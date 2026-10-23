#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd -P)"

# Lowercase RFC 4648 base32 of the first 100 bits of the checkout path's
# SHA-256: 25 hex digits, encoded 20 bits (4 characters) at a time.
workspace_name() {
  local alphabet=abcdefghijklmnopqrstuvwxyz234567 digest name="" i bits chunk
  digest="$(printf '%s' "${REPO_ROOT}" | sha256sum)"
  for ((i = 0; i < 25; i += 5)); do
    chunk=$((16#${digest:i:5}))
    for ((bits = 15; bits >= 0; bits -= 5)); do
      name+="${alphabet:chunk >> bits & 31:1}"
    done
  done
  printf 'workspace_%s\n' "${name}"
}

WORKSPACE_NAME="$(workspace_name)"
IMAGE_NAME="${IMAGE_NAME:-winprint-windows-test:latest}"
CONTAINER_NAME="${CONTAINER_NAME:-winprint-windows-vm}"
CACHE_DIR="${XDG_CACHE_HOME:-${HOME}/.cache}/winprint.rs/windows-vm"
COMMAND=""
CLIENT_ARGS=()
EXEC_ARGS=()
AUTO_STOP=0
PRUNE_ALL=0
VERBOSE=0
CONTAINER_RUNTIME="docker"
if command -v podman >/dev/null 2>&1; then
  CONTAINER_RUNTIME="podman"
fi

print_usage() {
  cat <<'USAGE'
Usage: run.sh [options] [start|stop|test|prune]
       run.sh [options] exec COMMAND [ARG ...]
       run.sh [options] ssh [ssh-args ...]
       run.sh [options] scp [scp-args ...]

Without a command, start the VM and run tests, leaving it running.
  start         Create the container if needed, then start the VM.
  stop          Shut down Windows and remove the container, interrupting any runs.
  test          Start the VM if needed, upload this checkout, and run tests.
  exec          Start the VM if needed, upload this checkout, and run a command.
  prune         Destroy the VM and its guest data; retain boxes and image.
  ssh           Run the host ssh client against the VM.
  scp           Run the host scp client; address guest paths as windows:D:/path.

Arguments after ssh/scp are passed to the client unchanged.
exec preserves literal arguments; use pwsh -Command '...' for PowerShell scripts.

Options:
  --auto-stop   Stop and remove the container after test/exec (or no command),
                unless another run is still using it.
  --all         With prune, also remove the boxes cache and container image.
  --verbose     Show build and VM startup logs.
  -h, --help    Show this help message.
USAGE
}

usage_error() {
  echo "Error: $1" >&2
  print_usage >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) print_usage; exit 0 ;;
    --auto-stop) AUTO_STOP=1 ;;
    --all) PRUNE_ALL=1 ;;
    --verbose) VERBOSE=1 ;;
    start|stop|test|prune|exec|ssh|scp)
      [[ -z "${COMMAND}" ]] || usage_error "specify only one command."
      COMMAND="$1"
      if [[ "${COMMAND}" == exec ]]; then
        shift
        EXEC_ARGS=("$@")
        break
      elif [[ "${COMMAND}" == ssh || "${COMMAND}" == scp ]]; then
        shift
        CLIENT_ARGS=("$@")
        break
      fi
      ;;
    *) usage_error "unknown argument: $1" ;;
  esac
  shift
done

if [[ "${AUTO_STOP}" -eq 1 && -n "${COMMAND}" &&
  "${COMMAND}" != test && "${COMMAND}" != exec ]]; then
  usage_error "--auto-stop is only valid with test, exec, or no command."
fi
if [[ "${PRUNE_ALL}" -eq 1 && "${COMMAND}" != prune ]]; then
  usage_error "--all is only valid with prune."
fi
if [[ "${COMMAND}" == exec && "${#EXEC_ARGS[@]}" -eq 0 ]]; then
  usage_error "exec requires a command."
fi

# Checkout sources and host CLI edits never invalidate the VM or container.
# Guest provisioning inputs recreate the VM and delete its guest data.
VM_INPUTS=(Vagrantfile)
# Image inputs only recreate the container; the VM is retained.
CONTAINER_INPUTS=(Dockerfile Vagrantfile entrypoint-in-container.sh exec-in-container.sh)
CONTAINER_FINGERPRINT_LABEL=winprint.container-fingerprint

# Relative filenames keep fingerprints checkout-independent.
fingerprint() (
  cd "${SCRIPT_DIR}"
  sha256sum "$@" | sha256sum | awk '{print $1}'
)

# Serialize lifecycle transitions. Never hold this lock while waiting for
# Windows, so stop/prune can always interrupt a slow start.
# Keep lock files outside the directories removed by prune.
lock_lifecycle() {
  mkdir -p "$(dirname "${CACHE_DIR}")"
  exec 8>"${CACHE_DIR}.lock"
  flock 8
}

unlock_lifecycle() {
  exec 8>&-
}

# Runs hold a shared usage lock until they exit, so recreation and --auto-stop
# from other checkouts leave their VM alone; explicit stop/prune interrupt it.
# The lock is only acquired under the lifecycle lock, so probing it there is
# race-free.
hold_usage() {
  exec 7>"${CACHE_DIR}.usage.lock"
  flock --shared 7
}

release_usage() {
  exec 7>&-
}

vm_in_use() (
  exec 6>"${CACHE_DIR}.usage.lock"
  ! flock --nonblock 6
)

# Empty when the container does not exist, but fail on runtime errors.
container_state() {
  local state
  if state="$(${CONTAINER_RUNTIME} container inspect --format '{{.State.Status}}' "${CONTAINER_NAME}" 2>/dev/null)"; then
    printf '%s\n' "${state}"
  else
    ${CONTAINER_RUNTIME} container ls >/dev/null || return
  fi
}

vm_ready() {
  [[ "$(container_state)" == running ]] &&
    ${CONTAINER_RUNTIME} exec "${CONTAINER_NAME}" test -f /run/vm-ready 2>/dev/null
}

require_ready() {
  if ! vm_ready; then
    echo "Error: VM is not ready. Run '$0 start' first." >&2
    exit 1
  fi
}

prepare_cache() {
  mkdir -p "${CACHE_DIR}"
  # Disable CoW on the cache directory to avoid performance issues with libvirt storage.
  # Ignore errors if the filesystem does not support it (e.g., XFS without reflink).
  chattr +C "${CACHE_DIR}" 2>/dev/null || true
  mkdir -p "${CACHE_DIR}"/{vagrant-home,vagrant-project,libvirt-data,libvirt-config}
}

remove_vm_state() {
  local paths=(vagrant-project libvirt-data libvirt-config vm-fingerprint vm-fingerprint.tmp ssh-known-hosts vagrant-home/data/machine-index)
  if [[ "${PRUNE_ALL}" -eq 1 ]]; then
    paths+=(vagrant-home)
  fi
  [[ -d "${CACHE_DIR}" ]] || return 0

  # Docker creates root-owned files; rootless Podman can create mapped-UID
  # files. Delete using the same runtime/user namespace that created them.
  if ${CONTAINER_RUNTIME} image inspect "${IMAGE_NAME}" >/dev/null 2>&1; then
    ${CONTAINER_RUNTIME} run --rm --privileged \
      -v "${CACHE_DIR}:/cache" --workdir /cache --entrypoint /bin/rm \
      "${IMAGE_NAME}" -rf -- "${paths[@]}" || return
  else
    local path
    for path in "${paths[@]}"; do
      rm -rf -- "${CACHE_DIR:?}/${path}" || return
    done
  fi
}

build_image() {
  local args=(buildx build --load --file "${SCRIPT_DIR}/Dockerfile" --tag "${IMAGE_NAME}" "${SCRIPT_DIR}")
  echo "Building ${IMAGE_NAME}..."
  if [[ "${VERBOSE}" -eq 1 ]]; then
    ${CONTAINER_RUNTIME} "${args[@]}"
  else
    local output
    output="$(${CONTAINER_RUNTIME} "${args[@]}" 2>&1)" || { printf '%s\n' "${output}" >&2; return 1; }
  fi
}

container_fingerprint() {
  ${CONTAINER_RUNTIME} container inspect \
    --format "{{index .Config.Labels \"${CONTAINER_FINGERPRINT_LABEL}\"}}" "${CONTAINER_NAME}"
}

create_container() {
  prepare_cache
  build_image
  ${CONTAINER_RUNTIME} create --name "${CONTAINER_NAME}" \
    --label "${CONTAINER_FINGERPRINT_LABEL}=$(fingerprint "${CONTAINER_INPUTS[@]}")" \
    --privileged --cgroupns=host --device /dev/kvm \
    --stop-timeout 300 \
    -v "${CACHE_DIR}/vagrant-home:/root/.vagrant.d" \
    -v "${CACHE_DIR}/vagrant-project:/app/.vagrant" \
    -v "${CACHE_DIR}/libvirt-data:/var/lib/libvirt" \
    -v "${CACHE_DIR}/libvirt-config:/etc/libvirt" \
    -e "VM_CPUS=${VM_CPUS:-$(nproc)}" \
    -e "VM_MEMORY=${VM_MEMORY:-$(awk '/MemTotal/ {print int($2 * 0.4 / 1024)}' /proc/meminfo)}" \
    "${IMAGE_NAME}" >/dev/null
}

# Recreate stale state, then start the container without waiting for Windows.
# Requires the lifecycle lock.
ensure_container() {
  local vm_fingerprint recorded="${CACHE_DIR}/vm-fingerprint"
  vm_fingerprint="$(fingerprint "${VM_INPUTS[@]}")"
  if ! [[ -f "${recorded}" && "$(<"${recorded}")" == "${vm_fingerprint}" ]]; then
    if [[ -d "${CACHE_DIR}/vagrant-project" || -n "$(container_state)" ]]; then
      if vm_in_use; then
        echo "Error: VM configuration changed, but the VM is in use by another run." >&2
        echo "Retry after it finishes, or run '$0 stop' to interrupt it." >&2
        return 1
      fi
      echo "VM configuration changed; recreating the VM."
      echo "Guest workspaces and build caches will be deleted; downloaded boxes are retained."
      stop_vm
      remove_vm_state
    fi
    # Record the inputs before provisioning starts. The container itself
    # restarts provisioning that failed or was interrupted.
    prepare_cache
    printf '%s\n' "${vm_fingerprint}" > "${recorded}.tmp"
    mv "${recorded}.tmp" "${recorded}"
  elif [[ -n "$(container_state)" &&
    "$(container_fingerprint)" != "$(fingerprint "${CONTAINER_INPUTS[@]}")" ]]; then
    if vm_in_use; then
      echo "Warning: container inputs changed, but the VM is in use by another run; reusing it." >&2
    else
      echo "Container inputs changed; recreating the container. The VM is retained."
      stop_vm
    fi
  fi
  [[ -n "$(container_state)" ]] || create_container
  # Podman's conmon outlives this script and inherits open descriptors; never
  # let it hold the lifecycle lock (flock follows the open file description).
  [[ "$(container_state)" == running ]] || ${CONTAINER_RUNTIME} start "${CONTAINER_NAME}" >/dev/null 8>&-
}

start_vm() {
  lock_lifecycle
  ensure_container
  hold_usage
  unlock_lifecycle
  wait_until_ready
}

wait_until_ready() {
  echo "Waiting for Windows VM (${CONTAINER_NAME})..."
  local log_pid=""
  if [[ "${VERBOSE}" -eq 1 ]]; then
    ${CONTAINER_RUNTIME} logs --follow --since 1s "${CONTAINER_NAME}" &
    log_pid=$!
  fi
  local deadline=$((SECONDS + ${VM_START_TIMEOUT:-7200})) result=0
  until vm_ready; do
    if [[ "$(container_state)" != running || "${SECONDS}" -ge "${deadline}" ]]; then
      echo "Error: VM startup failed or timed out." >&2
      result=1
      break
    fi
    sleep 2
  done
  if [[ -n "${log_pid}" ]]; then
    kill "${log_pid}" 2>/dev/null || true
    wait "${log_pid}" 2>/dev/null || true
  fi
  return "${result}"
}

stop_vm() {
  # Do not delete persistent state until container removal succeeds.
  local state
  state="$(container_state)" || return
  [[ -n "${state}" ]] || return 0
  if [[ "${state}" == running || "${state}" == paused ]]; then
    ${CONTAINER_RUNTIME} stop "${CONTAINER_NAME}" >/dev/null || return
  fi
  ${CONTAINER_RUNTIME} rm "${CONTAINER_NAME}" >/dev/null
}

stop_vm_locked() (
  lock_lifecycle || return
  stop_vm
)

prune_vm() (
  lock_lifecycle
  stop_vm
  remove_vm_state
  if [[ "${PRUNE_ALL}" -eq 1 ]]; then
    if ${CONTAINER_RUNTIME} image inspect "${IMAGE_NAME}" >/dev/null 2>&1; then
      ${CONTAINER_RUNTIME} image rm "${IMAGE_NAME}" >/dev/null
    fi
    echo "Removed VM state, boxes cache, and image ${IMAGE_NAME}."
  else
    echo "Removed VM state; boxes cache and container image retained."
  fi
)

stop_unused_vm() (
  lock_lifecycle || return
  if vm_in_use; then
    echo "The VM is in use by another run; leaving it running."
    return 0
  fi
  stop_vm
)

stop_after_command() {
  local result=$?
  trap - EXIT
  release_usage
  if ! stop_unused_vm; then
    echo "Error: could not stop/remove VM container." >&2
    if [[ "${result}" -eq 0 ]]; then result=1; fi
  fi
  exit "${result}"
}

run_exec() {
  start_vm
  require_ready
  # All commands share the same upload, workspace lock, and guest environment.
  tar -C "${REPO_ROOT}" --exclude=./target --exclude=./.git -cf - . |
    ${CONTAINER_RUNTIME} exec -i "${CONTAINER_NAME}" /app/exec-in-container.sh \
      "${WORKSPACE_NAME}" "$@"
}

run_tests() {
  run_exec cargo hack nextest run --all-targets --each-feature --skip default,test-utils --no-fail-fast
}

run_host_client() (
  lock_lifecycle
  hold_usage
  unlock_lifecycle
  require_ready
  # The guest network is private to the container. The host runs SSH/SCP;
  # ${CONTAINER_RUNTIME} exec only relays raw TCP inside that network.
  local connection_dir
  connection_dir="$(mktemp -d)"
  # A subshell keeps cleanup scoped to this connection, including setup errors.
  trap 'rm -rf "${connection_dir}"' EXIT
  ${CONTAINER_RUNTIME} cp "${CONTAINER_NAME}:/run/windows-ssh-config" "${connection_dir}/guest-config" 2>/dev/null
  ${CONTAINER_RUNTIME} cp "${CONTAINER_NAME}:/app/.vagrant/windows-ssh" "${connection_dir}/identity" 2>/dev/null
  chmod 600 "${connection_dir}/identity"
  {
    echo 'Host windows'
    awk '$1 == "HostName" || $1 == "Port" || $1 == "User"' "${connection_dir}/guest-config"
    cat <<CONFIG
  SetEnv WORKSPACE_PATH=D:\\${WORKSPACE_NAME}
  IdentityFile "${connection_dir}/identity"
  IdentitiesOnly yes
  BatchMode yes
  ConnectTimeout 5
  StrictHostKeyChecking no
  UserKnownHostsFile "${CACHE_DIR}/ssh-known-hosts"
  HostKeyAlias ${CONTAINER_NAME}-windows
  ProxyCommand ${CONTAINER_RUNTIME} exec -i "${CONTAINER_NAME}" nc %h %p
CONFIG
  } > "${connection_dir}/config"
  case "${COMMAND}" in
    ssh) ssh -F "${connection_dir}/config" windows "${CLIENT_ARGS[@]}" ;;
    scp) scp -F "${connection_dir}/config" "${CLIENT_ARGS[@]}" ;;
  esac
)

if [[ "${AUTO_STOP}" -eq 1 ]]; then
  trap stop_after_command EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
fi

case "${COMMAND}" in
  start) start_vm ;;
  stop) stop_vm_locked ;;
  prune) prune_vm ;;
  ""|test) run_tests ;;
  exec) run_exec "${EXEC_ARGS[@]}" ;;
  ssh|scp) run_host_client ;;
esac
