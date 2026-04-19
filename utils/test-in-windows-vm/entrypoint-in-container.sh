#!/usr/bin/env bash
set -euo pipefail

VERBOSE="${VERBOSE:-0}"

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

install_vagrant_plugin() {
    vagrant plugin install vagrant-libvirt
}

start_virt_services() {
    [ -e /dev/kvm ] && chown root:kvm /dev/kvm
    libvirtd --daemon
    virtlogd --daemon
    until virsh version &>/dev/null; do sleep 1; done
}

upload_workspace() {
    local upload_dir="$1"
    rsync -a \
        --exclude='target/' \
        --exclude='.git/' \
        /workspace/ "${upload_dir}/"
    vagrant upload "${upload_dir}" 'C:/workspace'
}

if ! vagrant plugin list 2>/dev/null | grep -q vagrant-libvirt; then
    run_stage "Installing vagrant-libvirt plugin" install_vagrant_plugin
fi

run_stage "Starting virtualization services" start_virt_services

if [ ! -e /dev/kvm ]; then
    export LIBVIRT_DRIVER="qemu"
    printf "Warning: KVM not available, falling back to QEMU (slow)\n"
fi

cd /app

run_stage "Setting up Windows VM" vagrant up --provider=libvirt

UPLOAD_DIR=$(mktemp -d /tmp/winprint-upload-XXXXX)
trap 'rm -rf "${UPLOAD_DIR}"' EXIT

run_stage "Cleaning up previous workspace on VM" vagrant provision --provision-with cleanup
run_stage "Uploading workspace to VM" upload_workspace "${UPLOAD_DIR}"

TEST_CODE=0
if ! run_stage "Running tests" vagrant provision --provision-with run-tests; then
    TEST_CODE=$?
fi

run_stage "Cleaning up workspace after tests" vagrant provision --provision-with cleanup
run_stage "Shutting down VM" vagrant halt

printf "Tests completed with exit code: %d\n" "${TEST_CODE}"
exit "${TEST_CODE}"
