#!/bin/sh
# Кнопка годинника WayVR: вимикає мікрофон лише для Discord (тебе не чують у дзвінку,
# решта програм чує як завжди). Глушить запис Discord у PipeWire/PulseAudio, тому
# в самому Discord значок мікрофона не перекреслюється.
#   discord-mute.sh toggle    — перемкнути
#   discord-mute.sh is-muted  — код 0, якщо мікрофон Discord зараз вимкнено

streams() {
	# номер і стан (true/false) кожного запису мікрофона від Discord
	pactl -f json list source-outputs 2>/dev/null | jq -r '.[]
		| select((.properties["application.process.binary"] // "" | ascii_downcase | test("discord"))
			or (.properties["application.name"] // "" | test("WEBRTC VoiceEngine")))
		| "\(.index) \(.mute)"'
}

case "$1" in
toggle)
	list=$(streams)
	[ -z "$list" ] && exit 1
	# хоч один чує — глушимо всі, інакше вмикаємо всі
	if echo "$list" | grep -q ' false$'; then want=1; else want=0; fi
	echo "$list" | while read -r id _; do pactl set-source-output-mute "$id" "$want"; done
	;;
is-muted)
	list=$(streams)
	[ -n "$list" ] && ! echo "$list" | grep -q ' false$'
	;;
*)
	echo "використання: $0 toggle|is-muted" >&2
	exit 2
	;;
esac
