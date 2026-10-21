#!/usr/bin/env bash
set -euo pipefail
usage() {
    printf '%s\n' 'Usage: bash deploy/start.sh [--preview|--execute]' \
        'Default: print the commands. --execute: run them on the target board.'
}
if (( $# > 1 )); then usage >&2; exit 2; fi
case "${1:---preview}" in
    --preview)
        printf '%s\n' \
            'systemd-sysusers /usr/lib/sysusers.d/robot.conf /usr/lib/sysusers.d/padd.conf' \
            'systemctl daemon-reload' \
            'systemctl start robotd.service padd.service xgoduck-web.service'
        ;;
    --execute)
        systemd-sysusers /usr/lib/sysusers.d/robot.conf /usr/lib/sysusers.d/padd.conf
        systemctl daemon-reload
        systemctl start robotd.service padd.service xgoduck-web.service
        ;;
    --help|-h) usage ;;
    *) usage >&2; exit 2 ;;
esac
