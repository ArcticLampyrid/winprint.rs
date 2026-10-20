#!/usr/bin/env bash
set -euo pipefail
cd /app
worker=""

start_virt_services() {
  # /etc/libvirt is a persistent bind mount. Containerized libvirt must not
  # change ownership or write trusted xattrs on QEMU images.
  cat > /etc/libvirt/qemu.conf <<'CONF'
user = "root"
group = "root"
dynamic_ownership = 0
remember_owner = 0
CONF
  libvirtd --daemon
  virtlogd --daemon
  until virsh version &>/dev/null; do sleep 1; done
}

# Halts Windows on every exit, so restarting the container never boots from a
# hard power-off. Only docker stop (or a startup failure) ends this process.
shutdown_vm() {
  local result=$?
  trap - EXIT
  rm -f /run/vm-ready
  if [[ -n "${worker}" ]]; then
    kill "${worker}" 2>/dev/null || true
    wait "${worker}" 2>/dev/null || true
  fi
  vagrant halt || true
  exit "${result}"
}

# /run is not a tmpfs, so state from the previous boot survives a restart.
rm -f /run/vm-ready /run/windows-ssh-config
trap shutdown_vm EXIT
trap 'exit 0' TERM INT
start_virt_services

if ! vagrant plugin list | grep -q vagrant-libvirt; then
  vagrant plugin install vagrant-libvirt
fi
if [[ ! -f /app/.vagrant/windows-ssh ]]; then
  ssh-keygen -t ed25519 -N '' -f /app/.vagrant/windows-ssh
fi

# Provisioners only run on the first boot, so failed or interrupted
# provisioning cannot resume; start over from the box instead.
provisioned=/app/.vagrant/provisioned
if [[ ! -f "${provisioned}" && -f /app/.vagrant/machines/default/libvirt/id ]]; then
  echo "Previous provisioning did not finish; recreating the VM."
  vagrant destroy --force
fi

# Run as a child so PID 1 can handle docker stop during provisioning.
# By default a failed boot destroys the VM, discarding its provisioning.
vagrant up --provider=libvirt --no-destroy-on-error &
worker=$!
wait "${worker}"
worker=""
touch "${provisioned}"
{
  vagrant ssh-config
  printf '\nHost default\n  BatchMode yes\n  ConnectTimeout 5\n'
} > /run/windows-ssh-config
# WinRM can become available before the automatically-started SSH service.
ssh_deadline=$((SECONDS + 120))
until ssh -F /run/windows-ssh-config default 'exit 0'; do
  if [[ "${SECONDS}" -ge "${ssh_deadline}" ]]; then
    echo "Error: Windows SSH did not become ready." >&2
    exit 1
  fi
  sleep 2
done
touch /run/vm-ready
echo "Windows VM ready."

while true; do
  sleep infinity &
  worker=$!
  wait "${worker}"
done
