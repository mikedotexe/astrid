#!/usr/bin/env bash
# Root-only completion of the ICP host hardening prepared by the edge rollout.

set -euo pipefail

upgrade_os=0
reboot_host=0
for argument in "$@"; do
    case "$argument" in
        --upgrade-os) upgrade_os=1 ;;
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

expected_uuid="6b7d53e2-b6fc-4363-9add-d3111eb2ef7d"
actual_uuid="$(blkid -s UUID -o value /dev/sda1)"
if [[ "$actual_uuid" != "$expected_uuid" ]]; then
    printf 'error: /dev/sda1 UUID changed: expected %s, found %s\n' \
        "$expected_uuid" "$actual_uuid" >&2
    exit 1
fi
if ! mountpoint -q /media/data; then
    printf 'error: /media/data is not currently an independent mount point\n' >&2
    exit 1
fi

install -d -m 1777 /media/data/tmp
chmod 1777 /media/data/tmp

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="/etc/fstab.before-astrid-hardening-$stamp"
cp -a /etc/fstab "$backup"
temporary="$(mktemp /etc/fstab.astrid.XXXXXX)"
awk -v expected="UUID=$expected_uuid" -v replacement="UUID=$expected_uuid /media/data ext4 nosuid,nodev,nofail,x-systemd.device-timeout=30s 0 2" '
    $1 == "/dev/sda1" && ($2 == "/media/data" || $2 == "/media/data/") {
        print replacement
        matched += 1
        next
    }
    $1 == expected && ($2 == "/media/data" || $2 == "/media/data/") {
        print replacement
        matched += 1
        next
    }
    { print }
    END { if (matched != 1) exit 42 }
' /etc/fstab > "$temporary" || {
    status=$?
    rm -f "$temporary"
    printf 'error: expected exactly one SSD /media/data fstab row (status %s)\n' \
        "$status" >&2
    exit 1
}
chmod 0644 "$temporary"
findmnt --verify --tab-file "$temporary"
install -m 0644 "$temporary" /etc/fstab
rm -f "$temporary"

systemctl disable --now \
    avahi-daemon.service avahi-daemon.socket \
    cups.service cups.socket cups.path cups-browsed.service || true
systemctl daemon-reload

if (( upgrade_os == 1 )); then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get -y -o Dpkg::Options::=--force-confold full-upgrade
    apt-get -y autoremove
    apt-get clean
fi

printf 'host_hardening=complete\n'
printf 'fstab_backup=%s\n' "$backup"
printf 'ssd_uuid=%s\n' "$expected_uuid"
printf 'tmp_mode=%s\n' "$(stat -c %a /media/data/tmp)"
printf 'os_upgrade=%s\n' "$upgrade_os"
if (( reboot_host == 1 )); then
    printf 'reboot=starting\n'
    sync
    systemctl reboot
else
    printf 'next=return control to Codex for reboot acceptance\n'
fi
