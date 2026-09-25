# AOG firmware 2.0.0

`sensor-kit/` and `dual-ovf/` share `libraries/AogDevice`. Both detect optional water probes on D6/D8. `package/` holds embedded HEX files, a SHA-256 manifest, and source archive. Bootloader images are excluded.

## Reproduce builds

Install [Arduino CLI 1.5.1](https://github.com/arduino/arduino-cli/releases/tag/v1.5.1), verify its release checksum, and create a CLI configuration for your development directories:

```sh
python3 firmware/build.py --cli /path/to/arduino-cli --config /path/to/arduino-cli.yaml --install
```

Dependencies are pinned: AVR core 1.8.6, OneWire 2.3.8, CCS811 1.1.3, DHT 1.4.6, BusIO 1.17.4, and Adafruit Unified Sensor 1.1.15. The script builds both sketches for Nano, Nano old bootloader, and Uno; then recreates checksums and `sources.zip`. Downloaded dependencies retain upstream licenses.

The Pi installer `bash deploy/install-firmware-tools.sh` installs CLI and AVR tools without flashing. Dashboard uploads use [Arduino CLI upload](https://docs.arduino.cc/arduino-cli/commands-reference/arduino-cli_upload) with a prebuilt HEX and verification.

## Memory and hardware

The RAM ring retains 24 one-minute samples; no histories are written to EEPROM. ATmega328P builds use approximately 1.5 KB of its 2 KB SRAM. Inspect compiler memory reports after changes. Native ring tests are in `tests/ram_buffer_test.cpp`.

Resets and power loss erase RAM; overflow reports drops. Water probes must be externally powered DS18B20-family devices with pull-ups. The actual probes and tank mapping remain to be established. Analog pH/gas voltages cannot verify physical probe presence. Verify pH calibration after replacing legacy firmware.

[Wiring](../docs/wiring.md) · [Protocol](../docs/firmware-protocol.md) · [Tests](../TESTING.md)

The Dual OVF firmware labels its nominal pH conversion `PH_STATUS: analog_unverified`.
That voltage does not prove a connected Gravity board or probe. AOG requires
physical connection confirmation in Connections before using this input as pH;
older unverified records remain raw diagnostics. For Atlas Gravity V2.1, wire
A to A0, + to 5V and − to GND on Dual OVF, then check known calibration buffers.
