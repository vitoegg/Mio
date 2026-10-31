include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-mio
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

PKG_LICENSE:=Apache-2.0
PKG_MAINTAINER:=Mio Maintainer

LUCI_TITLE:=LuCI Support for Mio (Shadowsocks Server)
LUCI_DESCRIPTION:=LuCI web interface for managing Mio (Shadowsocks Server).
LUCI_DEPENDS:=@x86_64 +luci-base +rpcd-mod-ucode +mio
LUCI_PKGARCH:=all

define Package/luci-app-mio/conffiles
/etc/config/mio
endef

include $(TOPDIR)/feeds/luci/luci.mk

# call BuildPackage - OpenWrt buildroot signature
