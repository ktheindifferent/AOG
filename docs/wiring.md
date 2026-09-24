# Connections and water-probe identification

**Connections** is the live source of board identity, firmware, last report, pin assignments, sensor errors, and raw values. Arduino D/A labels refer to the selected Arduino, not Pi GPIO.

| Signal | Sensor Kit | Dual OVF |
| --- | --- | --- |
| D2 | Unused | Overflow switch 1 to GND |
| D4 | Unused | Overflow switch 2 to GND |
| D6 | Optional water data | Optional water data |
| D7 | DHT11 data | Unused |
| D8 | Optional water data | Optional water data |
| A0 | Raw analog gas signal | Analog pH interface signal |
| A4 / A5 | CCS811 SDA / SCL | Unused |
| USB | Raspberry Pi | Raspberry Pi |

Overflow contacts close to ground when safe. Open/high blocks assigned pumps; software cannot distinguish overflow from broken wiring. Use the pH analog interface; do not connect the electrode directly to A0. Analog voltages are unverified connections.

The Qwiic LCD and relay board use Pi I²C: physical pin 3 SDA/GPIO2, pin 5 SCL/GPIO3, pin 1 3.3 V, pin 6 GND. Assign relay functions in Setup. SDS011 has a separate USB serial interface; cameras appear on Cameras.

## Unknown three-wire probes on D6 and D8

1. Keep D6/D8 names until tank assignments are established. Firmware checks both pins on both Arduinos.
2. Disconnect power before rewiring. Identify the model and wire functions from markings or manufacturer documentation; colors are not proof.
3. For a **confirmed externally powered DS18B20**, connect VDD to 5 V, GND to common ground, and DQ to D6 or D8. Each dedicated data line needs a 4.7 kΩ pull-up to 5 V. Parasite power is rejected.
4. ROM family `28` identifies DS18B20. ROM and scratchpad CRCs and a completed conversion are required. Two devices on one pin report `multiple_probes` instead of choosing one.
5. Name the probe after its tank and optionally select its Arduino. Automatic selection returns no value if both boards report valid probes on that pin.

`not_detected` is not a model diagnosis: check power, ground, data, pull-up, cable, and model. `crc_error` omits a corrupt sample. `unsupported_family` exposes the ROM without pretending it is DS18B20 data. `external_power_required` needs powered three-wire wiring. Missing sensors never produce inferred temperatures.

The DS18B20 has a unique 64-bit ROM, 1-Wire communication, and −55 to +125 °C range. See the [manufacturer's datasheet](https://www.analog.com/en/products/ds18b20.html) and [official Nano pinout](https://docs.arduino.cc/resources/pinouts/A000005-full-pinout.pdf).

CCS811 supply/logic requirements depend on the breakout; a bare chip is not a 5 V module. Firmware retries failed initialization without blocking other sensors. Its CO₂ field is **equivalent CO₂**, not calibrated direct CO₂. The unidentified A0 gas module remains raw ADC counts and is no longer averaged into that value.

Dual OVF emits raw ADC, nominal millivolts (5 V reference), and Atlas Gravity nominal pH conversion. Pi calibration is retained; verify it after upgrading, especially if the Arduino previously held EEPROM calibration. Firmware v2 does not write EEPROM. [Atlas analog pH documentation](https://atlas-scientific.com/embedded-solutions/surveryor-analog-ph-sensor-meter/).
