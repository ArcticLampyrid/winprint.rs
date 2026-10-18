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

configure_winrm_proxy_bypass() {
    local network_cidr="100.117.60.48/28"
    local network_hosts

    network_hosts=$(
        ruby -ripaddr -rsocket -e '
            range = IPAddr.new(ARGV.fetch(0)).to_range
            (range.begin.to_i + 1).upto(range.end.to_i - 1) do |address|
                puts IPAddr.new(address, Socket::AF_INET)
            end
        ' "${network_cidr}" |
            paste -sd, -
    )

    local bypass="${no_proxy:-${NO_PROXY:-}}"
    bypass="${bypass:+${bypass},}${network_hosts}"

    export no_proxy="${bypass}"
    export NO_PROXY="${bypass}"
}

start_virt_services() {
    # The libvirt configuration directory is bind-mounted by run.sh, so the
    # image's qemu.conf is hidden. Containerized libvirt cannot change
    # ownership or write the trusted xattrs it normally uses for QEMU files.
    local qemu_conf=/etc/libvirt/qemu.conf
    for setting in \
        'user = "root"' \
        'group = "root"' \
        'dynamic_ownership = 0' \
        'remember_owner = 0'; do
        local name="${setting%% *}"
        if grep -qE "^[[:space:]]*#?[[:space:]]*${name}[[:space:]]*=" "${qemu_conf}" 2>/dev/null; then
            sed -i -E "s|^[[:space:]]*#?[[:space:]]*${name}[[:space:]]*=.*|${setting}|" "${qemu_conf}"
        else
            printf '\n%s\n' "${setting}" >> "${qemu_conf}"
        fi
    done

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

configure_winrm_proxy_bypass
run_stage "Setting up Windows VM" vagrant up --provider=libvirt

UPLOAD_DIR=$(mktemp -d /tmp/winprint-upload-XXXXX)
trap 'rm -rf "${UPLOAD_DIR}"' EXIT

run_stage "Cleaning up previous workspace on VM" vagrant provision --provision-with cleanup
run_stage "Uploading workspace to VM" upload_workspace "${UPLOAD_DIR}"

TEST_CODE=0
run_stage "Running tests" vagrant provision --provision-with run-tests || TEST_CODE=$?

run_stage "Cleaning up workspace after tests" vagrant provision --provision-with cleanup
run_stage "Shutting down VM" vagrant halt

printf "Tests completed with exit code: %d\n" "${TEST_CODE}"
exit "${TEST_CODE}"
