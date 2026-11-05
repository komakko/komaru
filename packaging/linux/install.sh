#!/bin/sh
set -eu

repository=komakko/komaru
data=${XDG_DATA_HOME:-$HOME/.local/share}
bin_dir=$HOME/.local/bin
apps_dir=$data/applications
icon_dir=$data/icons/hicolor/256x256/apps
state_dir=$data/komaru
record=$state_dir/installed-files

if [ -n "${KOMARU_BASE_URL:-}" ]; then
    base=$KOMARU_BASE_URL
elif [ -n "${KOMARU_VERSION:-}" ]; then
    base=https://github.com/$repository/releases/download/$KOMARU_VERSION
else
    base=https://github.com/$repository/releases/latest/download
fi

fail() {
    echo "komaru: $*" >&2
    exit 1
}

fetch() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 -o "$2" "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$2" "$1"
    else
        fail "curl or wget is required"
    fi
}

refresh_caches() {
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps_dir" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

uninstall() {
    [ -f "$record" ] || fail "nothing to uninstall"
    while IFS= read -r path; do
        rm -f -- "$path"
    done <"$record"
    rm -f -- "$record"
    rmdir "$state_dir" 2>/dev/null || true
    refresh_caches
    echo "komaru removed. Settings in ${XDG_CONFIG_HOME:-$HOME/.config}/komaru were kept."
}

install_files() {
    [ "$(uname -s)" = Linux ] || fail "this installer is for Linux"
    [ "$(uname -m)" = x86_64 ] || fail "only x86_64 is supported"

    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT

    for name in komaru-x86_64-linux komaru.desktop komaru.png SHA256SUMS; do
        fetch "$base/$name" "$work/$name"
    done

    listed=$(grep -E '  (komaru-x86_64-linux|komaru\.desktop|komaru\.png)$' "$work/SHA256SUMS")
    [ "$(printf '%s\n' "$listed" | wc -l)" -eq 3 ] || fail "checksum list is incomplete"
    (cd "$work" && printf '%s\n' "$listed" | sha256sum -c --quiet -) || fail "checksum mismatch"

    mkdir -p "$bin_dir" "$apps_dir" "$icon_dir" "$state_dir"

    install -m 755 "$work/komaru-x86_64-linux" "$bin_dir/komaru.new"
    mv -f "$bin_dir/komaru.new" "$bin_dir/komaru"

    sed "s|^Exec=komaru |Exec=$bin_dir/komaru |" "$work/komaru.desktop" >"$apps_dir/komaru.desktop"
    install -m 644 "$work/komaru.png" "$icon_dir/komaru.png"

    printf '%s\n' "$bin_dir/komaru" "$apps_dir/komaru.desktop" "$icon_dir/komaru.png" "$record" >"$record"
    refresh_caches

    echo "komaru installed to $bin_dir/komaru"
    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *) echo "add $bin_dir to PATH to run it from a terminal" ;;
    esac
}

case "${1:-}" in
    "") install_files ;;
    --uninstall) uninstall ;;
    *) fail "usage: install.sh [--uninstall]" ;;
esac
