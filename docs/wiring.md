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

CCS811 supply/logic requirements depend on the breakout; a bare chip is not a 5 V module. Firmware retries failed initialization without blocking other sensors. Its CO₂ field is **equivalent CO₂**, not calibrated direct CO₂. An unidentified A0 gas module remains raw ADC counts. An identified DFRobot SEN0219 v1 analog infrared module can be converted to ppm and selected as the main CO₂ source in Connections; it is never averaged with CCS811 eCO₂.

## Diagnosing a low CO₂ reading

Connections compares two different signals explicitly: **CCS811 eCO₂ in ppm** and **A0 ADC counts**. Choose a board, then a sensor's **Trace wiring** button to follow its signal, power and ground connections. A reporting USB link does not prove each attached sensor works. Optional, undetected water probes appear separately from reported sensor errors.

`CO2_ADC: 160` means 160 out of 1023 ADC counts, approximately **781 mV** using the nominal 5 V reference (`160 × 5000 / 1024`). It is not 160 ppm. Actual reference voltage follows the board's supply, so the displayed voltage is an estimate. Near-zero or near-full-scale inputs deserve a voltage/wiring check; an in-range value still cannot prove a module is connected.

The legacy Sensor Kit sketch in Git history used `(mV - 400) × 3.125`, matching the **DFRobot SEN0219 v1 analog** example. For that specific module, 160 counts would be about **1,191 ppm**, before supply/calibration error. Select that profile only for the analog version of the module. **SEN0219 v2 uses PWM**, which requires a different acquisition method; do not apply the analog formula to it. See [DFRobot's version-specific explanation](https://www.dfrobot.com/forum/topic/315493).

Firmware 2.0.0 reports CCS811 eCO₂ separately and stores A0 as raw data. Controller 2.0.1 converts that raw data using the configured sensor profile. The main CO₂ reading uses one selected source, with no averaging or automatic fallback between inputs. A displayed 400 ppm may be the CCS811's own reporting floor. CCS811 estimates eCO₂ from gas response rather than measuring CO₂ directly. The previous sketch also wrote a fixed CCS811 baseline (`0x847B`) and passed unchecked DHT data to its compensation; v2 omits that unverified baseline and requires valid temperature/humidity. Sensor initialization, environmental compensation and an independent meter can therefore produce differing numbers. Follow [the CCS811 manufacturer's breakout guidance](https://learn.adafruit.com/adafruit-ccs811-air-quality-sensor/overview) for initial burn-in and warm-up.

In Connections → Sensor profile & CO₂ source, choose DFRobot SEN0219 v1 analog infrared, select the infrared source, and enter the measured ADC reference voltage (5.000 V nominal). Verify the module's rated power/common ground, compare A0-to-GND voltage with a meter, and follow its calibration procedure with a suitable reference. Do not force the raw number to agree with a CCS811 estimate. Existing history is preserved. Infrared chart/report statistics only use samples tagged with the infrared source, so older CCS811/legacy values do not pull the average down. CSV/JSON exports retain all original values and their source labels. CCS811 history can include older unassigned/legacy values and excludes infrared values. Source/reference settings are captured when a buffered record is first absorbed; duplicates and later setting changes never reinterpret that record.

The authenticated `GET /api/connections` response includes `co2_diagnostics`: source, freshness, digital eCO₂, raw ADC, reference-based millivolts, converted analog ppm, compensation availability and startup warm-up indication. `POST /api/co2-settings` requires the admin session and CSRF token and stores model, reference voltage and primary source in `co2_settings.json`. Stale/failed reports have no live gas values. `/api/stats` includes `co2_source` so Overview can label the reported value. Raw board frames remain available for inspection with their original timestamp.

Dual OVF emits raw ADC, nominal millivolts (5 V reference), and Atlas Gravity nominal pH conversion. Pi calibration is retained; verify it after upgrading, especially if the Arduino previously held EEPROM calibration. Firmware v2 does not write EEPROM. [Atlas analog pH documentation](https://atlas-scientific.com/embedded-solutions/surveryor-analog-ph-sensor-meter/).
