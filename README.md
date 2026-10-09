# Mio

A LuCI app that runs a Snell server on OpenWrt, so services on the home network can be reached from the public internet.

## Packages

- `luci-app-mio` - LuCI page and procd service: Snell service switch, listen port, password, firewall allow and log level.
- `snell-server` - Prebuilt Snell v5.0.1 server, bundled with the glibc runtime it needs on musl systems.

Requires an x86_64 OpenWrt build with musl.

## Client

The server enables HTTP obfuscation. Configure the Surge client with `version=5, obfs=http` and any `obfs-host`.

## Acknowledgments

- [Snell](https://kb.nssurge.com/surge-knowledge-base/release-notes/snell) - A lean encrypted proxy protocol by Surge
