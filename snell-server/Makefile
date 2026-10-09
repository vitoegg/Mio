include $(TOPDIR)/rules.mk

PKG_NAME:=snell-server
PKG_VERSION:=5.0.1
PKG_RELEASE:=1

PKG_SOURCE:=snell-server-v$(PKG_VERSION)-linux-amd64.zip
PKG_SOURCE_URL:=https://dl.nssurge.com/snell
PKG_HASH:=9bea1c2b9e35b73b31634856c04d18c393072b9e5dcde6a32781d8b8f908c539

PKG_LICENSE:=Proprietary LGPL-2.1-or-later GPL-3.0-or-later WITH GCC-exception-3.1
PKG_MAINTAINER:=Mio Maintainer

include $(INCLUDE_DIR)/package.mk

# snell-server is a UPX-packed glibc program: keep it unstripped and ship the
# Debian 12 runtime it links against. snapshot.debian.org keeps the pinned files
# after they leave the Debian pool.
RSTRIP:=:

GLIBC_DEB:=libc6_2.36-9+deb12u14_amd64.deb
LIBSTDCXX_DEB:=libstdc++6_12.2.0-14+deb12u1_amd64.deb
LIBGCC_DEB:=libgcc-s1_12.2.0-14+deb12u1_amd64.deb

define Download/glibc
  URL:=https://deb.debian.org/debian/pool/main/g/glibc \
	https://snapshot.debian.org/archive/debian/20260504T025922Z/pool/main/g/glibc
  FILE:=$(GLIBC_DEB)
  HASH:=ba4f88f73dbc3ae9055f3c20f4523bfdbaf1ad13ff95e258924f77d20b4fbedf
endef
$(eval $(call Download,glibc))

define Download/libstdcxx
  URL:=https://deb.debian.org/debian/pool/main/g/gcc-12 \
	https://snapshot.debian.org/archive/debian/20250415T084322Z/pool/main/g/gcc-12
  FILE:=$(LIBSTDCXX_DEB)
  HASH:=5cd3171216d4ab0fc911cfe9c35509bf2dd8f47761c43b7f6a4296701551a24d
endef
$(eval $(call Download,libstdcxx))

define Download/libgcc
  URL:=https://deb.debian.org/debian/pool/main/g/gcc-12 \
	https://snapshot.debian.org/archive/debian/20250415T084322Z/pool/main/g/gcc-12
  FILE:=$(LIBGCC_DEB)
  HASH:=3016e62cb4b7cd8038822870601f5ed131befe942774d0f745622cc77d8a88f7
endef
$(eval $(call Download,libgcc))

define Package/snell-server
  SECTION:=net
  CATEGORY:=Network
  SUBMENU:=Web Servers/Proxies
  TITLE:=Snell server (prebuilt, with glibc runtime)
  URL:=https://kb.nssurge.com/surge-knowledge-base/release-notes/snell
  DEPENDS:=@x86_64 @USE_MUSL
endef

define Package/snell-server/description
Prebuilt snell-server binary from Surge, bundled with the glibc runtime it needs on musl systems.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)/rootfs
	unzip -q -o $(DL_DIR)/$(PKG_SOURCE) -d $(PKG_BUILD_DIR)
	cd $(PKG_BUILD_DIR) && $(TARGET_AR) p $(DL_DIR)/$(GLIBC_DEB) data.tar.xz | $(TAR) -xJf - -C rootfs
	cd $(PKG_BUILD_DIR) && $(TARGET_AR) p $(DL_DIR)/$(LIBSTDCXX_DEB) data.tar.xz | $(TAR) -xJf - -C rootfs
	cd $(PKG_BUILD_DIR) && $(TARGET_AR) p $(DL_DIR)/$(LIBGCC_DEB) data.tar.xz | $(TAR) -xJf - -C rootfs
endef

Build/Compile:=

# The loader path is fixed by the binary (/lib64 links to /lib); Debian's loader
# searches /usr/lib/x86_64-linux-gnu, which musl programs never look at.
define Package/snell-server/install
	$(INSTALL_DIR) $(1)/usr/bin $(1)/lib $(1)/usr/lib/x86_64-linux-gnu
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/snell-server $(1)/usr/bin/snell-server
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2 $(1)/lib/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/libc.so.6 $(1)/usr/lib/x86_64-linux-gnu/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/libdl.so.2 $(1)/usr/lib/x86_64-linux-gnu/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/libm.so.6 $(1)/usr/lib/x86_64-linux-gnu/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/libpthread.so.0 $(1)/usr/lib/x86_64-linux-gnu/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/lib/x86_64-linux-gnu/libgcc_s.so.1 $(1)/usr/lib/x86_64-linux-gnu/
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/rootfs/usr/lib/x86_64-linux-gnu/libstdc++.so.6.0.30 $(1)/usr/lib/x86_64-linux-gnu/libstdc++.so.6
endef

$(eval $(call BuildPackage,snell-server))
