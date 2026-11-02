#!/bin/sh
# Кнопка годинника WayVR для голосової помічниці «Фрея» (~/вайп-кодерка/watch-claude):
#   freya.sh press — почати слухати одразу, без імені (запускає слухачку, якщо вона вимкнена)
#   freya.sh busy  — код 0, поки Фрея слухає, думає або говорить (кнопка світиться)

status="${XDG_RUNTIME_DIR:-/tmp}/freya-status.json"

case "$1" in
press)
	if systemctl --user -q is-active freya-listener; then
		systemctl --user kill --kill-whom=main -s USR1 freya-listener
	else
		touch "${XDG_RUNTIME_DIR:-/tmp}/freya-wake"
		systemctl --user start freya-listener
	fi
	;;
busy)
	[ -f "$status" ] || exit 1
	# свіжий стан (слухачка жива) і зайнята
	jq -e '(now - .time) < 3 and .busy' "$status" >/dev/null 2>&1
	;;
*)
	echo "використання: $0 press|busy" >&2
	exit 2
	;;
esac
