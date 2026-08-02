#!/usr/bin/env bash
# Minimal host hardening for the AVADO Astrid appliance.

set -euo pipefail

reboot_host=0
for argument in "$@"; do
    case "$argument" in
        --reboot) reboot_host=1 ;;
        *)
            printf 'error: unknown option: %s\n' "$argument" >&2
            exit 2
            ;;
    esac
done

if (( EUID != 0 )); then
    printf 'error: run this script through sudo\n' >&2
    exit 1
fi
if [[ "$(hostname -s)" != "avado" ]]; then
    printf 'error: this helper is intended only for the AVADO host\n' >&2
    exit 1
fi

. /etc/os-release
if [[ "${ID:-}" != "debian" || "${VERSION_ID:-}" != "13" ]]; then
    printf 'error: expected Debian 13, found %s %s\n' \
        "${ID:-unknown}" "${VERSION_ID:-unknown}" >&2
    exit 1
fi
if [[ "$(findmnt -no UUID /)" != "d755722c-35a2-414d-ba17-9dd80a56a741" ]]; then
    printf 'error: unexpected filesystem mounted at /\n' >&2
    exit 1
fi
if ! loginctl show-user avado -p Linger --value | grep -qx yes; then
    printf 'error: avado user lingering is not enabled\n' >&2
    exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get -y -o Dpkg::Options::=--force-confold full-upgrade
apt-get -y install ufw
apt-get -y autoremove
apt-get clean

# Retain the useful ICP builder image and stopped container, but do not keep a
# privileged container daemon running on a dedicated Astrid appliance.
# Stop the socket first so it cannot reactivate Docker while the service is
# being retired. Inactive units do not need reset-failed; asking systemd to
# reset an unloaded healthy unit only produces a misleading warning.
systemctl disable --now docker.socket
systemctl disable --now docker.service
systemctl disable --now containerd.service

# Astrid's model and reservoir ports are already loopback-only. Keep SSH as the
# sole general inbound service and make that posture explicit at the firewall.
ufw default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw --force enable

printf 'host_hardening=complete\n'
printf 'kernel=%s\n' "$(uname -r)"
printf 'ufw=%s\n' "$(ufw status | sed -n '1p')"
printf 'docker_images=retained\n'
printf 'docker_runtime=disabled\n'

if (( reboot_host == 1 )); then
    printf 'reboot=starting\n'
    sync
    systemctl reboot
else
    printf 'next=rerun with --reboot after reviewing this output\n'
fi
