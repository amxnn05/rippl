#!/bin/sh
# Builds rippl and installs it.
#
#   scripts/install.sh                 into ~/.local (no root needed)
#   scripts/install.sh /usr            into /usr (run with sudo)
#   DESTDIR=pkg scripts/install.sh /usr   into a package's folder (PKGBUILD)
#   scripts/install.sh --uninstall [prefix]
#
# Lays out: <prefix>/bin/rippl, <prefix>/lib/rippl/librippl_web_extension.so,
# <prefix>/share/applications/dev.rippl.Rippl.desktop, the icons under
# <prefix>/share/icons/hicolor, and the AppStream metadata.
set -eu
cd "$(dirname "$0")/.."

uninstall=0
if [ "${1:-}" = "--uninstall" ]; then uninstall=1; shift; fi
prefix="${1:-$HOME/.local}"
dest="${DESTDIR:-}"
root="$dest$prefix"

files() {
    echo "bin/rippl"
    echo "lib/rippl/librippl_web_extension.so"
    echo "share/applications/dev.rippl.Rippl.desktop"
    echo "share/metainfo/dev.rippl.Rippl.metainfo.xml"
    (cd data/icons && find hicolor -type f) | sed 's|^|share/icons/|'
}

if [ "$uninstall" = 1 ]; then
    files | while read -r f; do rm -f "$root/$f"; done
    rmdir "$root/lib/rippl" 2>/dev/null || true
    echo "rippl removed from $prefix (your settings and data in ~/.config/rippl and ~/.local/share/rippl stay)"
    exit 0
fi

if [ "${RIPPL_SKIP_BUILD:-0}" != 1 ]; then
    cargo build --release --locked -p rippl -p rippl-web-extension
fi
install -Dm755 target/release/rippl "$root/bin/rippl"
install -Dm644 target/release/librippl_web_extension.so "$root/lib/rippl/librippl_web_extension.so"
install -Dm644 data/dev.rippl.Rippl.desktop "$root/share/applications/dev.rippl.Rippl.desktop"
install -Dm644 data/dev.rippl.Rippl.metainfo.xml "$root/share/metainfo/dev.rippl.Rippl.metainfo.xml"
(cd data/icons && find hicolor -type f) | while read -r f; do
    install -Dm644 "data/icons/$f" "$root/share/icons/$f"
done

# Installed for real (not into a package's folder): tell the desktop.
if [ -z "$dest" ]; then
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$prefix/share/icons/hicolor" 2>/dev/null || true
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
fi
echo "rippl installed in $root"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) [ -z "$dest" ] && echo "note: $prefix/bin isn't on your PATH" ;;
esac
