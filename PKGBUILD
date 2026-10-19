# Maintainer: WMDE <https://wmde.fun>
# Contributor: System76 <jeremy@system76.com> (original cosmic-edit)
# Builds our fork Lin-WMDE/wmde-edit (branch wmde; master mirrors pop-os upstream).
# WMDE Notepad: fork of cosmic-edit cut down to a notepad and rebranded to fun.wmde.Edit,
# built against ../libcosmic (fun.wmde config) and using ../wmde-files's file dialog (compiled
# in, not a runtime package dep). build.rs/xdgen was dropped; res/ templates are installed
# directly.
pkgname=wmde-edit
pkgver=1.10.3
pkgrel=1
pkgdesc="WMDE Notepad - plain text editor for the WMDE desktop (fork of cosmic-edit)"
arch=('x86_64')
url="https://wmde.fun"
license=('GPL-3.0-only')
# !debug: skip the split-debug package for this large statically-linked libcosmic binary.
# !lto: makepkg's lto option puts -flto into CFLAGS, the cc crate compiles the oniguruma C
# library of syntax highlighting (onig_sys) with it, gcc emits LTO bytecode objects and
# rustc's ld.lld sees every onig_* symbol as undefined. Rust's own LTO is unaffected: it
# comes from the release profile. cargo check cannot catch this, only the package build.
options=('!lto' '!debug')
# depends: readelf-NEEDED core libs + the libcosmic runtime set (wayland/mesa/fontconfig/
# freetype2 are dlopen'd by libcosmic at runtime). Verify with namcap.
depends=('glibc' 'gcc-libs' 'glib2' 'libxkbcommon' 'wayland' 'mesa' 'fontconfig' 'freetype2')
optdepends=('wmde-icons-base: WMDE icon theme'
            'polkit: saving files that belong to root (pkexec)')
# makedepends: same toolchain/libs that build the fork in Docker + glib2.
makedepends=('rust' 'cargo' 'git' 'clang' 'lld' 'pkgconf' 'glib2' 'mesa' 'wayland'
             'libxkbcommon' 'fontconfig' 'freetype2' 'expat' 'zstd')
source=("$pkgname::git+https://github.com/Lin-WMDE/wmde-edit.git#branch=wmde")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/$pkgname"
  # WMDE version: <COSMIC major.minor>.<WMDE release>.<commits since the nearest tag>.g<short>.
  local desc
  desc=$(git describe --long --tags --abbrev=7 2>/dev/null || true)
  if [ -n "$desc" ]; then
    printf '1.10.3.%s.g%s' "$(printf '%s' "$desc" | sed -E 's/.*-([0-9]+)-g[0-9a-f]+$/\1/')" "$(git rev-parse --short=7 HEAD)"
  else
    printf '1.10.3.%s.g%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
  fi
}

build() {
  cd "$srcdir/$pkgname"
  # x86-64-v3 (AVX2/BMI2) baseline for the WMDE repo; runs on Haswell+ (and the VM).
  export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=x86-64-v3"
  # link the system libzstd; the vendored zstd-sys build drops symbols under lld
  export ZSTD_SYS_USE_PKG_CONFIG=1
  cargo build --release --bin wmde-edit
}

check() {
  cd "$srcdir/$pkgname"
  # The dev profile on purpose: the release profile aborts on panic, and the test harness
  # needs to unwind to report a failure. No pipe after cargo: makepkg runs check() without
  # pipefail, and a pipe would swallow a failing test run.
  export ZSTD_SYS_USE_PKG_CONFIG=1
  cargo test --locked
}

package() {
  cd "$srcdir/$pkgname"
  local appid=fun.wmde.Edit
  install -Dm755 "${CARGO_TARGET_DIR:-target}/release/wmde-edit" "$pkgdir/usr/bin/wmde-edit"
  install -Dm644 "res/$appid.desktop" "$pkgdir/usr/share/applications/$appid.desktop"
  install -Dm644 "res/$appid.metainfo.xml" "$pkgdir/usr/share/metainfo/$appid.metainfo.xml"
  # The key bindings the notepad handles itself, so that Settings can offer a page for them
  # without knowing anything about the notepad.
  install -Dm644 "res/app-shortcuts/$appid.ron" \
    "$pkgdir/usr/share/wmde/app-shortcuts/$appid.ron"
  # App icons (all sizes) for the icon theme lookup. The icon name is the application id,
  # which is also what the window reports - a mismatch leaves the task bar button blank.
  for icon in res/icons/hicolor/*/apps/"$appid".svg; do
    local size
    size=$(basename "$(dirname "$(dirname "$icon")")")
    install -Dm644 "$icon" "$pkgdir/usr/share/icons/hicolor/$size/apps/$appid.svg"
  done
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
