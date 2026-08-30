include $(TOPDIR)/rules.mk

PKG_NAME:=shadowsocks-rust-ssserver
PKG_VERSION:=1.25.0
PKG_RELEASE:=1

PKG_SOURCE:=shadowsocks-v$(PKG_VERSION).x86_64-unknown-linux-musl.tar.xz
PKG_SOURCE_URL:=https://github.com/shadowsocks/shadowsocks-rust/releases/download/v$(PKG_VERSION)
PKG_HASH:=8439bf43c324b0fc273e663d0b1f8926fd8f666cbd1e0fd59b35096f0e778e92

PKG_LICENSE:=MIT
PKG_MAINTAINER:=Mio Maintainer

include $(INCLUDE_DIR)/package.mk

define Package/shadowsocks-rust-ssserver
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=Web Servers/Proxies
  TITLE:=shadowsocks-rust ssserver
  URL:=https://github.com/shadowsocks/shadowsocks-rust
  DEPENDS:=@TARGET_x86_64
endef

define Package/shadowsocks-rust-ssserver/description
Prebuilt ssserver binary from the upstream shadowsocks-rust release.
endef

define Build/Prepare
	$(TAR) -C $(PKG_BUILD_DIR) -xJf $(DL_DIR)/$(PKG_SOURCE)
endef

Build/Compile:=

define Package/shadowsocks-rust-ssserver/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/ssserver $(1)/usr/bin/ssserver
endef

$(eval $(call BuildPackage,shadowsocks-rust-ssserver))
