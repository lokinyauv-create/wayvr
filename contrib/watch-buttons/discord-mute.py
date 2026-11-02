#!/usr/bin/env python3
"""Кнопка годинника WayVR: вимикає мікрофон у самому Discord (як кнопка 🎤 у Discord —
усі бачать перекреслений мікрофон). Працює через місцевий канал керування Discord (RPC).

    discord-mute.py setup     — один раз: підключити (Discord попросить «Авторизувати»)
    discord-mute.py toggle    — перемкнути мікрофон
    discord-mute.py is-muted  — код 0, якщо мікрофон у Discord вимкнено

Для RPC потрібен свій «застосунок» Discord (Developer Portal → New Application).
Його Client ID і Client Secret лежать у discord-mute.conf.json поруч (тільки для тебе, 0600):
    {"client_id": "...", "client_secret": "..."}
У застосунку в OAuth2 → Redirects має бути http://localhost
"""

import json
import os
import socket
import struct
import sys
import time
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
CONF = HERE / "discord-mute.conf.json"
TOKEN = HERE / "discord-mute.token.json"
REDIRECT = "http://localhost"
SCOPES = ["rpc", "rpc.voice.read", "rpc.voice.write"]

OP_HANDSHAKE, OP_FRAME = 0, 1


def sockets():
    run = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
    for base in (f"{run}/app/com.discordapp.Discord", run, f"{run}/snap.discord", "/tmp"):
        for i in range(10):
            yield f"{base}/discord-ipc-{i}"


class Rpc:
    def __init__(self, client_id):
        self.sock = None
        for path in sockets():
            if not os.path.exists(path):
                continue
            try:
                s = socket.socket(socket.AF_UNIX)
                s.settimeout(5)
                s.connect(path)
                self.sock = s
                break
            except OSError:
                continue
        if not self.sock:
            raise SystemExit("Discord не запущений (немає discord-ipc)")
        self.send(OP_HANDSHAKE, {"v": 1, "client_id": client_id})
        ready = self.recv()
        if ready.get("evt") != "READY":
            raise SystemExit(f"Discord відмовив: {ready}")

    def send(self, op, payload):
        data = json.dumps(payload).encode()
        self.sock.sendall(struct.pack("<II", op, len(data)) + data)

    def recv(self):
        head = self._read(8)
        op, length = struct.unpack("<II", head)
        return json.loads(self._read(length))

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise SystemExit("Discord закрив з'єднання")
            buf += chunk
        return buf

    def call(self, cmd, args=None, timeout=5):
        nonce = str(uuid.uuid4())
        self.send(OP_FRAME, {"cmd": cmd, "args": args or {}, "nonce": nonce})
        self.sock.settimeout(timeout)
        while True:
            msg = self.recv()
            if msg.get("nonce") == nonce:
                if msg.get("evt") == "ERROR":
                    raise RuntimeError(msg["data"].get("message", msg["data"]))
                return msg.get("data") or {}


def load_conf():
    try:
        conf = json.loads(CONF.read_text())
        return conf["client_id"], conf["client_secret"]
    except (OSError, KeyError, ValueError):
        raise SystemExit(f"Немає налаштувань: {CONF}")


def ask_conf():
    """Питає Client ID і Secret, якщо їх ще немає, і зберігає тільки для тебе (0600)."""
    try:
        conf = json.loads(CONF.read_text())
    except (OSError, ValueError):
        conf = {}
    if conf.get("client_id") and conf.get("client_secret"):
        return
    conf["client_id"] = input("Встав Client ID і натисни Enter: ").strip()
    conf["client_secret"] = input("Встав Client Secret і натисни Enter: ").strip()
    CONF.write_text(json.dumps(conf, indent=2))
    CONF.chmod(0o600)
    TOKEN.unlink(missing_ok=True)


def token_request(fields):
    req = urllib.request.Request(
        "https://discord.com/api/oauth2/token",
        data=urllib.parse.urlencode(fields).encode(),
        headers={"Content-Type": "application/x-www-form-urlencoded",
                 "User-Agent": "DiscordBot (wayvr-watch-button, 1.0)"},
    )
    with urllib.request.urlopen(req, timeout=15) as resp:
        tok = json.loads(resp.read())
    tok["expires_at"] = time.time() + tok.get("expires_in", 0) - 60
    TOKEN.write_text(json.dumps(tok))
    TOKEN.chmod(0o600)
    return tok


def authorize(rpc, client_id, secret):
    """Discord показує вікно «Авторизувати» — натиснути один раз."""
    code = rpc.call("AUTHORIZE", {"client_id": client_id, "scopes": SCOPES}, timeout=120)["code"]
    return token_request({"client_id": client_id, "client_secret": secret,
                          "grant_type": "authorization_code", "code": code,
                          "redirect_uri": REDIRECT})


def connect(allow_authorize=False):
    client_id, secret = load_conf()
    rpc = Rpc(client_id)
    tok = None
    try:
        tok = json.loads(TOKEN.read_text())
    except (OSError, ValueError):
        pass
    if tok and tok.get("expires_at", 0) < time.time() and tok.get("refresh_token"):
        try:
            tok = token_request({"client_id": client_id, "client_secret": secret,
                                 "grant_type": "refresh_token",
                                 "refresh_token": tok["refresh_token"]})
        except OSError:
            tok = None
    if not tok:
        if not allow_authorize:
            raise SystemExit("Ще не підключено: запусти discord-mute.py setup")
        tok = authorize(rpc, client_id, secret)
    try:
        rpc.call("AUTHENTICATE", {"access_token": tok["access_token"]})
    except RuntimeError:
        if not allow_authorize:
            raise SystemExit("Доступ скасовано: запусти discord-mute.py setup")
        tok = authorize(rpc, client_id, secret)
        rpc.call("AUTHENTICATE", {"access_token": tok["access_token"]})
    return rpc


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "setup":
        ask_conf()
        print("Зараз у Discord з'явиться вікно — натисни «Авторизувати».")
        connect(allow_authorize=True)
        print("Готово: кнопка Discord на годиннику підключена.")
    elif cmd == "toggle":
        rpc = connect()
        muted = rpc.call("GET_VOICE_SETTINGS").get("mute", False)
        rpc.call("SET_VOICE_SETTINGS", {"mute": not muted})
    elif cmd == "is-muted":
        sys.exit(0 if connect().call("GET_VOICE_SETTINGS").get("mute") else 1)
    else:
        print(__doc__)
        sys.exit(2)


if __name__ == "__main__":
    main()
