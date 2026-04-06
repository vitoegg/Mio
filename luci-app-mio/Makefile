include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-mio
PKG_VERSION:=0.0.2

PKG_LICENSE:=Apache-2.0
PKG_MAINTAINER:=Mio Maintainer

LUCI_TITLE:=LuCI Support for Mio (Shadowsocks-Rust Server)
LUCI_DESCRIPTION:=LuCI web interface for managing Mio (Shadowsocks-Rust Server).
LUCI_DEPENDS:=+luci-base +rpcd-mod-ucode +shadowsocks-rust-ssserver +simple-obfs-server
LUCI_PKGARCH:=all

# Minimum supported OpenWrt version: 23.05
# This package uses ucode-based rpcd backend which requires OpenWrt 21.02+
# and rpcd-mod-ucode which is stable since OpenWrt 23.05
PKG_CONFIG_DEPENDS:=CONFIG_VERSION_NUMBER

include $(TOPDIR)/feeds/luci/luci.mk

define Package/luci-app-mio/conffiles
/etc/config/mio
endef

$(eval $(call BuildPackage,luci-app-mio))
