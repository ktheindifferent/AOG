#!/bin/bash
# Read-only smoke checks for a running Rust v2 server with LAN bindings.
set -euo pipefail

device_ip=${1:-$(hostname -I | awk '{print $1}')}
if [ -z "$device_ip" ]; then
    echo "No network address found. Pass the AOG device IP as the first argument." >&2
    exit 1
fi
read -r https_port command_port < <(python3 - <<'CONFIG'
import json
with open('/opt/aog/data.json') as config_file:
    config = json.load(config_file)
print(int(config.get('https_bind_port') or 8443),
      int(config.get('command_api_bind_port') or 9443))
CONFIG
)

check_status() {
    local expected=$1 url=$2
    shift 2
    local status
    # The device uses a self-signed certificate. This test checks routing/auth.
    status=$(curl --noproxy '*' -k -sS --connect-timeout 3 --max-time 10 -o /dev/null -w '%{http_code}' "$@" "$url")
    if [ "$status" != "$expected" ]; then
        echo "FAIL $url: expected HTTP $expected, got $status" >&2
        return 1
    fi
    echo "PASS $url: HTTP $status"
}

check_status 200 "https://127.0.0.1:$https_port/login.html"
check_status 200 "https://$device_ip:$https_port/login.html"
check_status 302 "https://$device_ip:$https_port/index.html"
check_status 200 "https://$device_ip:$https_port/api/stats"
check_status 401 "https://$device_ip:$command_port/api/command" --data-urlencode 'input_command=help'
