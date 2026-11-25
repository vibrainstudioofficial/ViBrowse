#!/usr/bin/env bash
set -euo pipefail

# ViBrowse V1.2.1 — Native Linux (.deb & .rpm) Builder
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$PROJECT_DIR/release/linux"
BUILD_ROOT="/tmp/vibrowse-pkg-build"
export CARGO_TARGET_DIR="/tmp/vibrowse-linux-target"

APP_NAME="vibrowse"
DISPLAY_NAME="ViBrowse"
DEB_VERSION="1.2.1"
RPM_VERSION="1.2.1"
ARCH_DEB="amd64"
ARCH_RPM="x86_64"

mkdir -p "$OUT_DIR" "$BUILD_ROOT"

echo "[1/4] Building optimized Linux x86_64 release binary..."
cd "$PROJECT_DIR"
cargo build --release

BIN_PATH="$CARGO_TARGET_DIR/release/$APP_NAME"
if [[ ! -f "$BIN_PATH" ]]; then
  echo "Error: Binary not found at $BIN_PATH" >&2
  exit 1
fi
strip --strip-all "$BIN_PATH" || true

# Desktop entry & icon staging helper
create_desktop_file() {
  local dest="$1"
  cat > "$dest" <<EOF
[Desktop Entry]
Name=ViBrowse
Comment=Lightweight, low-RAM Chrome-compatible web browser built with Rust
Exec=/usr/bin/vibrowse %U
Icon=vibrowse
Terminal=false
Type=Application
Categories=Network;WebBrowser;
MimeType=text/html;text/xml;application/xhtml+xml;x-scheme-handler/http;x-scheme-handler/https;x-scheme-handler/magnet;
StartupWMClass=vibrowse
EOF
}

echo "[2/4] Packaging Debian/Ubuntu (.deb)..."
DEB_STAGE="$BUILD_ROOT/deb/${APP_NAME}_${DEB_VERSION}_${ARCH_DEB}"
rm -rf "$DEB_STAGE"
mkdir -p \
  "$DEB_STAGE/DEBIAN" \
  "$DEB_STAGE/usr/bin" \
  "$DEB_STAGE/usr/share/applications" \
  "$DEB_STAGE/usr/share/icons/hicolor/32x32/apps" \
  "$DEB_STAGE/usr/share/icons/hicolor/128x128/apps"

cp "$BIN_PATH" "$DEB_STAGE/usr/bin/$APP_NAME"
chmod 0755 "$DEB_STAGE/usr/bin/$APP_NAME"
create_desktop_file "$DEB_STAGE/usr/share/applications/$APP_NAME.desktop"
cp "$PROJECT_DIR/icons/32x32.png" "$DEB_STAGE/usr/share/icons/hicolor/32x32/apps/$APP_NAME.png"
cp "$PROJECT_DIR/icons/128x128.png" "$DEB_STAGE/usr/share/icons/hicolor/128x128/apps/$APP_NAME.png"

INSTALLED_KB="$(du -sk "$DEB_STAGE/usr" | awk '{print $1}')"
cat > "$DEB_STAGE/DEBIAN/control" <<EOF
Package: $APP_NAME
Version: $DEB_VERSION
Section: web
Priority: optional
Architecture: $ARCH_DEB
Installed-Size: $INSTALLED_KB
Depends: libwebkit2gtk-4.1-0, libgtk-3-0
Maintainer: ViBrowse <team@vibrowse.local>
Description: ViBrowse V1.1.a — Lightweight Rust Web Browser
 Lightweight, low-RAM web browser with Auto Tab Sleeping, built-in Ad & Popup
 Blocker, 16-connection IDM Download Manager, Video Sniffer, and native Rust
 BitTorrent client.
EOF

DEB_OUT="$OUT_DIR/ViBrowse-V1.2.1-linux-${ARCH_DEB}.deb"
dpkg-deb --root-owner-group -Zxz --build "$DEB_STAGE" "$DEB_OUT"

echo "[3/4] Packaging Fedora/RHEL/openSUSE (.rpm)..."
RPM_TOP="$BUILD_ROOT/rpmbuild"
rm -rf "$RPM_TOP"
mkdir -p "$RPM_TOP"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,STAGE}

cp "$BIN_PATH" "$RPM_TOP/STAGE/$APP_NAME"
chmod 0755 "$RPM_TOP/STAGE/$APP_NAME"
create_desktop_file "$RPM_TOP/STAGE/$APP_NAME.desktop"
cp "$PROJECT_DIR/icons/32x32.png" "$RPM_TOP/STAGE/icon-32.png"
cp "$PROJECT_DIR/icons/128x128.png" "$RPM_TOP/STAGE/icon-128.png"

cat > "$RPM_TOP/SPECS/$APP_NAME.spec" <<EOF
%global debug_package %{nil}
%define _build_id_links none

Name:           $APP_NAME
Version:        $RPM_VERSION
Release:        1
Summary:        ViBrowse V1.1.a — Lightweight Rust Web Browser
License:        GPL-3.0-or-later
URL:            https://vibrowse.local
BuildArch:      $ARCH_RPM
AutoReqProv:    no
Requires:       webkit2gtk4.1, gtk3

%description
Lightweight, low-RAM web browser built with Rust & Tauri v2 featuring Auto Tab
Sleeping, Ad & Smart Popup Blocker, 16-connection IDM Download Manager, Video
Sniffer, and native BitTorrent client.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/32x32/apps
mkdir -p %{buildroot}/usr/share/icons/hicolor/128x128/apps
install -m 0755 $RPM_TOP/STAGE/$APP_NAME %{buildroot}/usr/bin/$APP_NAME
install -m 0644 $RPM_TOP/STAGE/$APP_NAME.desktop %{buildroot}/usr/share/applications/$APP_NAME.desktop
install -m 0644 $RPM_TOP/STAGE/icon-32.png %{buildroot}/usr/share/icons/hicolor/32x32/apps/$APP_NAME.png
install -m 0644 $RPM_TOP/STAGE/icon-128.png %{buildroot}/usr/share/icons/hicolor/128x128/apps/$APP_NAME.png

%files
/usr/bin/$APP_NAME
/usr/share/applications/$APP_NAME.desktop
/usr/share/icons/hicolor/32x32/apps/$APP_NAME.png
/usr/share/icons/hicolor/128x128/apps/$APP_NAME.png
EOF

rpmbuild --define "_topdir $RPM_TOP" -bb "$RPM_TOP/SPECS/$APP_NAME.spec"
RPM_BUILT="$(find "$RPM_TOP/RPMS" -name '*.rpm' | head -n 1)"
RPM_OUT="$OUT_DIR/ViBrowse-V1.2.1-linux-${ARCH_RPM}.rpm"
cp -f "$RPM_BUILT" "$RPM_OUT"

echo "[4/4] Done! Linux packages saved to $OUT_DIR:"
ls -lh "$DEB_OUT" "$RPM_OUT"
