#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

IMAGE_NAME="${IMAGE_NAME:-winprint-windows-test:latest}"
XDG_CACHE_HOME="${XDG_CACHE_HOME:-${HOME}/.cache}"
CACHE_DIR="${XDG_CACHE_HOME}/winprint.rs/windows-vm"

DESTROY=0
VERBOSE=0

print_usage() {
  cat <<'USAGE'
Usage: run.sh [options]

Options:
  -h, --help    Show this help message.
  --destroy     Destroy the local VM.
  --verbose     Show command output in real-time.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      print_usage
      exit 0
      ;;
    --destroy)
      DESTROY=1
      shift
      ;;
    --verbose)
      VERBOSE=1
      shift
      ;;
    *)
      echo "Error: Unknown argument: $1" >&2
      print_usage >&2
      exit 2
      ;;
  esac
done

if [[ "${DESTROY}" -eq 1 ]]; then
  echo "Destroying cache: ${CACHE_DIR}"
  rm -rf "${CACHE_DIR}"
  exit $?
fi

mkdir -p \
  "${CACHE_DIR}/vagrant-home" \
  "${CACHE_DIR}/vagrant-project" \
  "${CACHE_DIR}/libvirt-data" \
  "${CACHE_DIR}/libvirt-config"

run_stage() {
    local title="$1"
    shift
    local exit_code=0
    if [[ "${VERBOSE}" -eq 1 ]]; then
        printf "==> %s\n" "${title}"
        "$@" || exit_code=$?
        if [ "${exit_code}" -ne 0 ]; then
            printf "==> %s FAILED (exit code: %d)\n" "${title}" "${exit_code}" >&2
            return "${exit_code}"
        fi
    else
        local output
        printf "%s..." "${title}"
        output=$("$@" 2>&1) || exit_code=$?
        if [ "${exit_code}" -eq 0 ]; then
            printf " done\n"
        else
            printf " FAILED\n"
            printf "%s\n" "${output}"
            return "${exit_code}"
        fi
    fi
}

BUILD_ARGS=(
  --load
  --file "${SCRIPT_DIR}/Dockerfile"
  --tag "${IMAGE_NAME}"
  "${SCRIPT_DIR}"
)

run_stage "Building Docker image" docker buildx build "${BUILD_ARGS[@]}"

VM_CPUS="${VM_CPUS:-$(nproc)}"
VM_MEMORY="${VM_MEMORY:-$(( $(grep MemTotal /proc/meminfo | awk '{print $2}') * 40 / 100 / 1024 ))}"

docker run --rm \
  --privileged \
  --cgroupns=host \
  --device /dev/kvm \
  -v "${CACHE_DIR}/vagrant-home:/root/.vagrant.d" \
  -v "${CACHE_DIR}/vagrant-project:/app/.vagrant" \
  -v "${CACHE_DIR}/libvirt-data:/var/lib/libvirt" \
  -v "${CACHE_DIR}/libvirt-config:/etc/libvirt" \
  -v "${REPO_ROOT}:/workspace:ro" \
  -e "VM_CPUS=${VM_CPUS}" \
  -e "VM_MEMORY=${VM_MEMORY}" \
  -e "VERBOSE=${VERBOSE}" \
  "${IMAGE_NAME}"
