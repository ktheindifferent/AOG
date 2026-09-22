#!/bin/sh
# Isolate runtime data and localhost ports from the running AOG service.
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
test_runtime=$(mktemp -d /tmp/aog-test-runtime.XXXXXX)
trap 'rm -rf -- "$test_runtime"' EXIT HUP INT TERM
mkdir -p "$test_runtime/sensors"
cargo_binary=$(command -v cargo)
test_user=$(id -un)
# Resolve dependencies before entering the isolated network namespace.
"$cargo_binary" fetch --locked

sudo unshare --mount --net --propagation private sh -c '
    set -eu
    mount --bind "$1" /opt/aog
    ip link set lo up
    cd "$2"
    test_user=$3
    cargo_binary=$4
    shift 4
    # Keep timing-sensitive sensor tests reliable on a busy, memory-limited Pi.
    exec runuser -u "$test_user" -- env CARGO_NET_OFFLINE=true RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}" "$cargo_binary" test --locked -j 1 "$@"
' sh "$test_runtime" "$project_dir" "$test_user" "$cargo_binary" "$@"
