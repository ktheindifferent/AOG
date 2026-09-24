#!/usr/bin/env bash
set -euo pipefail
# Install only upload tools; the controller includes precompiled firmware.
if [[ $(id -u) != 0 ]]; then exec sudo "$0" "$@"; fi
case "$(uname -m)" in aarch64) arch=ARM64;; x86_64) arch=64bit;; armv7l) arch=ARMv7;; *) echo 'Unsupported architecture' >&2; exit 1;; esac
version=1.5.1
directory=/opt/aog/tools
mkdir -p "$directory"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
base="https://github.com/arduino/arduino-cli/releases/download/v$version"
archive="arduino-cli_${version}_Linux_${arch}.tar.gz"
curl --fail --location --proto '=https' --tlsv1.2 "$base/$archive" --output "$temporary/$archive"
curl --fail --location --proto '=https' --tlsv1.2 "$base/$version-checksums.txt" --output "$temporary/checksums.txt"
(cd "$temporary" && grep " $archive\$" checksums.txt | sha256sum --check --strict)
tar -xzf "$temporary/$archive" -C "$temporary" arduino-cli
install -m 755 "$temporary/arduino-cli" "$directory/arduino-cli"
cat > "$directory/arduino-cli.yaml" <<'CONFIG'
directories:
  data: /opt/aog/tools/arduino-data
  downloads: /opt/aog/tools/arduino-downloads
  user: /opt/aog/tools/arduino-user
CONFIG
"$directory/arduino-cli" --config-file "$directory/arduino-cli.yaml" core update-index
"$directory/arduino-cli" --config-file "$directory/arduino-cli.yaml" core install arduino:avr@1.8.6
echo 'Arduino upload tools installed. No Arduino was flashed.'
