#!/usr/bin/env bash
# Retire the ICP appliance's obsolete vendor stack and EOL mainline kernel.

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

maintained_kernel="5.15.0-186-generic"
obsolete_kernel="6.0.0-060000-generic"

for required in \
    "/boot/vmlinuz-$maintained_kernel" \
    "/boot/initrd.img-$maintained_kernel" \
    "/lib/modules/$maintained_kernel"; do
    if [[ ! -e "$required" ]]; then
        printf 'error: maintained kernel prerequisite is absent: %s\n' "$required" >&2
        exit 1
    fi
done

if ! mountpoint -q /media/data; then
    printf 'error: refusing host changes while Astrid SSD is not mounted\n' >&2
    exit 1
fi
if [[ "$(findmnt -no UUID /media/data)" != "6b7d53e2-b6fc-4363-9add-d3111eb2ef7d" ]]; then
    printf 'error: unexpected filesystem mounted at /media/data\n' >&2
    exit 1
fi

# These belonged to the appliance's former role or live-image installation.
# Astrid does not depend on them, and Docker was already retired separately.
systemctl disable --now \
    groundseg.service \
    casper-md5check.service \
    casper.service \
    containerd.service || true
systemctl reset-failed \
    groundseg.service \
    casper-md5check.service \
    casper.service \
    containerd.service 2>/dev/null || true

# Preserve remote administration while making the inbound firewall posture
# explicit. Astrid's Ollama and reservoir listeners remain loopback-only.
ufw default deny incoming
ufw default allow outgoing
ufw allow OpenSSH
ufw --force enable

obsolete_packages=(
    linux-image-unsigned-6.0.0-060000-generic
    linux-modules-6.0.0-060000-generic
    linux-headers-6.0.0-060000-generic
    linux-headers-6.0.0-060000
)
installed_obsolete=()
for package in "${obsolete_packages[@]}"; do
    if dpkg-query -W -f='${db:Status-Status}' "$package" 2>/dev/null \
        | grep -qx installed; then
        installed_obsolete+=("$package")
    fi
done
if (( ${#installed_obsolete[@]} > 0 )); then
    export DEBIAN_FRONTEND=noninteractive
    apt-get -y purge "${installed_obsolete[@]}"
fi
update-grub

if [[ "$(readlink -f /boot/vmlinuz)" != "/boot/vmlinuz-$maintained_kernel" ]]; then
    printf 'error: /boot/vmlinuz does not select maintained kernel after GRUB update\n' >&2
    exit 1
fi
if [[ -e "/boot/vmlinuz-$obsolete_kernel" ]]; then
    printf 'error: obsolete kernel image remains after purge\n' >&2
    exit 1
fi

printf 'host_hardening_stage2=complete\n'
printf 'next_kernel=%s\n' "$maintained_kernel"
printf 'ufw=%s\n' "$(ufw status | sed -n '1p')"
printf 'retained_ssd_backup=%s\n' \
    /media/data/astrid/backups/emmc-20260729T130835Z

if (( reboot_host == 1 )); then
    printf 'reboot=starting\n'
    sync
    systemctl reboot
else
    printf 'next=rerun with --reboot after reviewing this output\n'
fi
