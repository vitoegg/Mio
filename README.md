# Mio

🏠 A Shadowsocks server for OpenWrt: reach your home network from anywhere.

- 🔐 Shadowsocks 2022 over TCP and UDP, with simple-obfs `http`
- 🦀 Built on shadowsocks-rust: one static binary of about 3 MB, x86_64 only
- 📦 `mio` is the prebuilt server; `luci-app-mio` adds the LuCI page and procd service

## 📱 Client

Shadowsocks 2022 with `obfs=http` (any host). Key: `openssl rand -base64 16` for `2022-blake3-aes-128-gcm`, `openssl rand -base64 32` for the others.

💡 TCP Fast Open: `net.ipv4.tcp_fastopen=3` in a file under `/etc/sysctl.d/`.

## 🚀 Release

Bump `PKG_VERSION` in `mio/Makefile` → push tag `v<PKG_VERSION>`. CI builds and releases the binary, keeping the latest 3, then commits its hash to `mio/Makefile`; a tag without a new version is skipped.

## 🙏 Thanks

[shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust) · [simple-obfs](https://github.com/shadowsocks/simple-obfs)
