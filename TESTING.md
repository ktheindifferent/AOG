# Testing AOG v2

Run commands from the repository root. Tests on the live Pi must use isolation: some integration tests exercise `/opt/aog` and localhost ports.

```sh
sh test-on-device.sh
```

The helper fetches locked dependencies, mounts a temporary runtime directory in a private mount namespace, creates an isolated network namespace, and runs Cargo as the calling user. Live runtime data stays outside these namespaces. Use `sh test-on-device.sh --lib` for the library suite.

On a disposable machine with no AOG service or hardware, create a writable `/opt/aog` and use `cargo test --locked -j1 -- --test-threads=1`.

## Firmware

```sh
c++ -std=c++11 -Wall -Wextra -Werror -fsanitize=address,undefined \
  firmware/tests/ram_buffer_test.cpp -o /tmp/aog-ram-buffer-test
/tmp/aog-ram-buffer-test
python3 firmware/build.py --install
```

The native test covers ring overflow, correct/incorrect ACKs, duplicate ACKs, and wraparound. Builds compile both sketches for Nano, Nano old bootloader, and Uno and report flash/SRAM use. Compiler success is not a physical hardware test.

Rust regressions cover durable storage before ACK, replay deduplication after restart, failed storage, invalid timestamps, avoiding live overflow/pH changes from history, water-source ambiguity, six-column history compatibility, water backfill in exports, and firmware control interlocks.

## Browser

Install Python Playwright and local Chromium. Tests simulate APIs and never operate physical outputs.

```sh
python3 -m venv /tmp/aog-browser-env
/tmp/aog-browser-env/bin/pip install playwright
/tmp/aog-browser-env/bin/python tests/web_ui_test.py
/tmp/aog-browser-env/bin/python tests/web_ui_hardware_test.py
```

Set `CHROMIUM` if the executable is not `/usr/bin/chromium`. Run browser suites sequentially on a Pi after backend builds and tests finish. Additional `tests/web_ui_*_test.py` suites cover settings conflicts, drafts, calibration, report options, charts, cameras, sessions, and output actions.

The hardware suite checks 1440, 390, and 320 pixel layouts, stale/failed reports, CRC errors, board switching, unsaved probe names, source selection, CSRF-protected update requests, and water history navigation. Screenshots go to `/tmp/aog-ui-hardware`; set `AOG_UI_ARTIFACTS` to retain them elsewhere.

Physical acceptance requires confirming actual probe models, pull-ups, tank assignments, USB reset behavior, and bootloader profiles. Firmware updates require an identified AOG board, confirmed-off outputs, and a matching packaged binary.
