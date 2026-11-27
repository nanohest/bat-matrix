#!/bin/sh
# Install the latest bat-matrix release from GitHub.
#
#   curl -fsSL https://raw.githubusercontent.com/nanohest/bat-matrix/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/nanohest/bat-matrix/main/install.sh | sh -s -- --no-service

set -eu

REPO=nanohest/bat-matrix
NAME=bat-matrix
ASSET=bat-matrix-x86_64-linux
SERVICE=bat-matrix.service

bin_dir=$HOME/.local/bin
install_service=1

usage() {
	cat <<EOF
Usage: install.sh [--bin-dir DIR] [--no-service]

Download the latest $NAME release from GitHub, verify it against the release
provenance if slsa-verifier is installed, and install it.

Options:
  --bin-dir DIR   Install the binary into DIR (default: ~/.local/bin)
  --no-service    Do not install and enable the systemd user service
  -h, --help      Show this help
EOF
}

info() { printf '%s\n' "$*" >&2; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

download() {
	curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1" || die "failed to download $1"
}

# Quote a path for use as the command in a systemd ExecStart= line.
systemd_quote() {
	printf '"%s"' "$(printf '%s' "$1" | sed 's/%/%%/g')"
}

while [ $# -gt 0 ]; do
	case $1 in
	--bin-dir)
		[ $# -ge 2 ] || die "--bin-dir requires an argument"
		bin_dir=$2
		shift 2
		;;
	--bin-dir=*)
		bin_dir=${1#*=}
		shift
		;;
	--no-service)
		install_service=0
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		usage >&2
		die "unknown option: $1"
		;;
	esac
done

case $bin_dir in
\~ | \~/*) bin_dir=$HOME${bin_dir#\~} ;;
esac
case $bin_dir in
/*) ;;
*) bin_dir=$PWD/$bin_dir ;;
esac
bin_dir=${bin_dir%/}

[ "$(uname -s)" = Linux ] || die "$NAME only supports Linux"
case $(uname -m) in
x86_64 | amd64) ;;
*) die "no prebuilt binary for $(uname -m); build $NAME from source instead" ;;
esac
command -v curl >/dev/null 2>&1 || die "curl is required"
if [ "$install_service" = 1 ]; then
	command -v systemctl >/dev/null 2>&1 ||
		die "systemctl not found; rerun with --no-service to install only the binary"
fi

latest_url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest") ||
	die "could not look up the latest release of $REPO"
case $latest_url in
*/releases/tag/*) tag=${latest_url##*/} ;;
*) die "no release found for $REPO" ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM

info "Downloading $NAME $tag..."
base_url=https://github.com/$REPO/releases/download/$tag
download "$base_url/$ASSET" "$tmp/$ASSET"

if command -v slsa-verifier >/dev/null 2>&1; then
	info "Verifying provenance..."
	download "$base_url/$ASSET.intoto.jsonl" "$tmp/$ASSET.intoto.jsonl"
	slsa-verifier verify-artifact "$tmp/$ASSET" \
		--provenance-path "$tmp/$ASSET.intoto.jsonl" \
		--source-uri "github.com/$REPO" \
		--source-tag "$tag" >&2 ||
		die "provenance verification failed, not installing $NAME"
else
	warn "slsa-verifier not found, skipping provenance verification"
	warn "install it from https://github.com/slsa-framework/slsa-verifier to verify releases"
fi

info "Installing $NAME to $bin_dir..."
if mkdir -p "$bin_dir" 2>/dev/null && [ -w "$bin_dir" ]; then
	install -m 755 "$tmp/$ASSET" "$bin_dir/$NAME"
elif command -v sudo >/dev/null 2>&1; then
	info "$bin_dir is not writable, using sudo"
	sudo install -D -m 755 "$tmp/$ASSET" "$bin_dir/$NAME"
else
	die "$bin_dir is not writable"
fi

case :$PATH: in
*:"$bin_dir":*) ;;
*) warn "$bin_dir is not on your PATH" ;;
esac

if [ "$install_service" = 1 ]; then
	unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
	info "Installing $SERVICE to $unit_dir..."
	download "https://raw.githubusercontent.com/$REPO/$tag/systemd/$SERVICE" "$tmp/$SERVICE"
	EXEC_START=$(systemd_quote "$bin_dir/$NAME") awk '
		/^ExecStart=/ { print "ExecStart=" ENVIRON["EXEC_START"]; next }
		{ print }
	' "$tmp/$SERVICE" >"$tmp/$SERVICE.new"
	mkdir -p "$unit_dir"
	install -m 644 "$tmp/$SERVICE.new" "$unit_dir/$SERVICE"

	systemctl --user daemon-reload ||
		die "could not reach the systemd user manager; rerun with --no-service to skip the service"
	systemctl --user enable "$SERVICE"
	systemctl --user restart "$SERVICE"

	info "$NAME $tag is installed and running."
	info "Check it with: systemctl --user status $SERVICE"
else
	info "$NAME $tag is installed to $bin_dir/$NAME."
fi
