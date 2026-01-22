include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-mio
PKG_VERSION:=0.0.1
PKG_RELEASE:=1

PKG_LICENSE:=Apache-2.0
PKG_MAINTAINER:=Mio Maintainer

LUCI_TITLE:=LuCI Support for Mio (Shadowsocks-Rust Server)
LUCI_DESCRIPTION:=LuCI web interface for managing Mio (Shadowsocks-Rust Server).
LUCI_DEPENDS:=+luci-base +shadowsocks-rust-ssserver +simple-obfs-server
LUCI_PKGARCH:=all

include $(TOPDIR)/feeds/luci/luci.mk

define Package/luci-app-mio/conffiles
/etc/config/mio
endef

$(eval $(call BuildPackage,luci-app-mio))
