include $(TOPDIR)/rules.mk

PKG_NAME:=mio
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

# Built from src/ and released by .github/workflows/mio.yml, which then writes PKG_HASH.
PKG_SOURCE:=mio-$(PKG_VERSION)-linux-amd64
PKG_SOURCE_URL:=https://github.com/vitoegg/Mio/releases/download/v$(PKG_VERSION)
PKG_HASH:=9ab1b010fb65f94ae9058b8f2d0685baaed7aca2144792a1ed613c12f308921d

PKG_LICENSE:=GPL-3.0-or-later
PKG_LICENSE_FILES:=src/LICENSE
PKG_MAINTAINER:=Mio Maintainer

include $(INCLUDE_DIR)/package.mk

# The binary is stripped at build time.
RSTRIP:=:

define Package/mio
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=Web Servers/Proxies
  TITLE:=Shadowsocks server built on shadowsocks-rust
  URL:=https://github.com/vitoegg/Mio
  DEPENDS:=@x86_64
  USERID:=mio:mio
endef

define Package/mio/description
Runs a Shadowsocks 2022 server of shadowsocks-rust behind HTTP obfuscation and sends all traffic out directly.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	$(CP) $(DL_DIR)/$(PKG_SOURCE) $(PKG_BUILD_DIR)/mio
endef

Build/Compile:=

define Package/mio/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/mio $(1)/usr/bin/mio
endef

$(eval $(call BuildPackage,mio))
