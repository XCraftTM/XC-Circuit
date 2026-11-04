#!/usr/bin/env bash
# Makes the Linux packages from an unpacked release folder:
#   packaging/linux/build-packages.sh <release folder> <version> <arch> <output folder>
# arch is x86_64 or aarch64. Writes a .deb, an .rpm and an .AppImage.
#
# Needs: dpkg-deb, rpmbuild (apt: rpm) and curl (to fetch appimagetool).
# Everything goes to /opt/xc-circuit, with a `xc-circuit` command, a menu
# entry and icons; the program finds plugins/ and examples/ next to itself.
set -euo pipefail

src=$(realpath "$1")
version=$2
arch=$3
out=$(realpath -m "$4")
here=$(dirname "$(realpath "$0")")
icons="$here/../icons"
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

case "$arch" in
  x86_64) deb_arch=amd64 ;;
  aarch64) deb_arch=arm64 ;;
  *) echo "unknown arch $arch" >&2; exit 1 ;;
esac

# The installed files, shared by .deb and .rpm.
root="$work/root"
mkdir -p "$root/opt/xc-circuit" "$root/usr/bin" "$root/usr/share/applications"
cp -r "$src"/. "$root/opt/xc-circuit/"
ln -s /opt/xc-circuit/xc-circuit "$root/usr/bin/xc-circuit"
cp "$here/xc-circuit.desktop" "$root/usr/share/applications/"
for size in 16 24 32 48 64 128 256 512; do
  dir="$root/usr/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$dir"
  cp "$icons/xc-circuit-$size.png" "$dir/xc-circuit.png"
done
chmod 755 "$root/opt/xc-circuit/xc-circuit"

summary="Build Discord and Fluxer bots by connecting blocks"
description="A visual editor for Discord and Fluxer bots: wire events, actions and values on a canvas, run the bot with one click, and export it as a Node.js project. Running bots needs Node.js 22 or newer (XC Circuit can install a private copy)."

# --- .deb -----------------------------------------------------------------
deb="$work/deb"
cp -a "$root" "$deb"
mkdir -p "$deb/DEBIAN"
size_kb=$(du -sk "$deb" | cut -f1)
cat > "$deb/DEBIAN/control" <<EOF
Package: xc-circuit
Version: $version
Section: devel
Priority: optional
Architecture: $deb_arch
Installed-Size: $size_kb
Maintainer: XCraftTM <xcrafttm@users.noreply.github.com>
Homepage: https://xcrafttm.github.io/XC-Circuit/
Depends: libgtk-3-0t64 | libgtk-3-0, libxkbcommon0, libasound2t64 | libasound2, libdbus-1-3, libgl1
Recommends: libvulkan1, nodejs
Description: $summary
 $description
EOF
dpkg-deb --root-owner-group --build "$deb" "$out/XC-Circuit-$version-linux-$arch.deb"

# --- .rpm -----------------------------------------------------------------
rpmtop="$work/rpm"
mkdir -p "$rpmtop"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cat > "$rpmtop/SPECS/xc-circuit.spec" <<EOF
Name: xc-circuit
Version: $version
Release: 1
Summary: $summary
License: Proprietary (XC Circuit Licence)
URL: https://xcrafttm.github.io/XC-Circuit/
Requires: gtk3, libxkbcommon, alsa-lib, dbus-libs, mesa-libGL
Recommends: vulkan-loader, nodejs
%global debug_package %{nil}
%global __os_install_post %{nil}
%global _build_id_links none

%description
$description

%install
cp -a $root/. %{buildroot}/

%files
/opt/xc-circuit
/usr/bin/xc-circuit
/usr/share/applications/xc-circuit.desktop
/usr/share/icons/hicolor/*/apps/xc-circuit.png
EOF
rpmbuild --define "_topdir $rpmtop" --target "$arch" -bb "$rpmtop/SPECS/xc-circuit.spec"
cp "$rpmtop"/RPMS/*/xc-circuit-*.rpm "$out/XC-Circuit-$version-linux-$arch.rpm"

# --- .AppImage --------------------------------------------------------------
app="$work/XC-Circuit.AppDir"
mkdir -p "$app/opt"
cp -r "$src" "$app/opt/xc-circuit"
cp "$here/xc-circuit.desktop" "$app/xc-circuit.desktop"
cp "$icons/xc-circuit-256.png" "$app/xc-circuit.png"
cat > "$app/AppRun" <<'EOF'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/opt/xc-circuit/xc-circuit" "$@"
EOF
chmod 755 "$app/AppRun"
tool="$work/appimagetool"
curl -fsSL -o "$tool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage"
chmod 755 "$tool"
# The runners have no FUSE: the tool unpacks itself instead.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$arch" "$tool" --no-appstream "$app" "$out/XC-Circuit-$version-linux-$arch.AppImage"

ls -la "$out"
