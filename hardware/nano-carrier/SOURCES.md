# Design sources and verification scope

Reviewed 2026-09-25. Manufacturer documentation takes precedence over a cable's
colors or an unlabeled breakout's apparent shape.

| Source | Design use |
| --- | --- |
| [Arduino classic Nano pinout](https://docs.arduino.cc/resources/pinouts/A000005-full-pinout.pdf) and [board documentation](https://docs.arduino.cc/hardware/nano) | Classic 5 V ATmega328P pin assignments; A4/A5 I²C; separate USB-powered Nano sockets |
| [Harwin M20-7821546](https://www.harwin.com/products/M20-7821546) | Female 15-way socket; 2.54 mm pitch, 8.5 mm body height, 38.5 × 2.5 mm body, 0.64 mm square mating pins |
| [METZ CONNECT Type 086](https://www.metz-connect.com/home/products/u-contact/pc-board-terminal-blocks/screw-type-terminal-blocks.7f.en.html?id=310861) | RT034xxHBLC screw terminals, 3.81 mm pitch, part-number aliases 31086102/03/04 |
| [DS18B20 datasheet](https://www.analog.com/media/en/technical-documentation/data-sheets/DS18B20.pdf) | Externally powered 1-Wire probe connection and per-bus pull-up |
| [Analog Devices pull-up guidance](https://support.analog.com/en-US/knowledgebase/article/000094969) | 4.7 kΩ pull-up required for powered 3.3 V / 5 V operation |
| [Atlas Gravity pH V2.1 datasheet](https://files.atlas-scientific.com/Gravity-pH-datasheet.pdf) | External analog pH interface, A/+/- wiring; electrode remains on its interface |
| [DFRobot SEN0219 version guidance](https://www.dfrobot.com/forum/topic/315493) | Analog v1 versus PWM v2; current firmware's analog profile is v1 only |
| [DHT11 documentation hosted by DFRobot](https://image.dfrobot.com/image/data/DFR0067/DFR0067_DS_10_en.pdf) | Single-wire DHT11 and data pull-up; excludes newer I²C replacement modules |
| [Adafruit CCS811 pinouts](https://learn.adafruit.com/adafruit-ccs811-air-quality-sensor/pinouts) | Example of breakout-specific power/logic and WAKE wiring; does not identify the user's breakout |
| [Nexperia AN10441](https://assets.nexperia.com/documents/application-note/AN10441.pdf) | Bidirectional MOSFET level shifting: gate to 3.3 V, source to low-voltage bus, drain to 5 V bus |
| [Diodes BSS138 datasheet](https://www.diodes.com/assets/Datasheets/BSS138.pdf) | SOT-23 pin assignment, low gate threshold, BSS138-7-F part |
| [Microchip MCP1700 datasheet](https://ww1.microchip.com/downloads/aemDocuments/documents/APID/ProductDocuments/DataSheets/MCP1700-Data-Sheet-20001826F.pdf) | 3.3 V regulator, SOT-23 pin 1 GND / 2 OUT / 3 IN; ceramic output capacitor requirement |

The KiCad PCB uses library footprints from Debian's **kicad-footprints
6.0.11-1** and embedded schematic symbols from **kicad-symbols 6.0.10-1**.
Their [upstream library license](https://www.kicad.org/libraries/license/)
permits use in electronic designs under its design exception. The project's
PCB/schematic files embed those definitions. No vendor datasheet PDF has been
redistributed in the order package.

The terminal footprints in `rev-a/AOG.pretty` are drawn from METZ CONNECT's
2025-08-21 Type 086 drawing: 3.81 mm pitch, 1.1 mm finished holes, 2.2 mm pads,
body length `(poles − 1) × 3.81 + 3.7 mm`, and 7.3 mm body depth. The stock
KiCad 6 footprint's 0.7 mm drill was rejected. The manufacturer's current
drill-pattern and mechanical drawings were inspected visually. The local
regulator symbol is a flattened KiCad MCP1700 symbol with the explicit 3.3 V
part name, so the project does not rely on inherited library-symbol resolution.

Electrical assignments were checked against the repository's installed
`firmware/libraries/AogDevice/src/AogDevice.h`, `firmware/sensor-kit/` and
`firmware/dual-ovf/`. No firmware assumptions were changed by this board.

Validation is explicitly separated:

- KiCad PCB DRC: clearance, drill separation, courtyard, silk, unconnected pads.
- Electrical contract: Nano pin mapping, connector nets, regulator and MOSFET
  pin identities, pull-up values, unused VIN/3V3/AREF, manufacturing origin.
- File checks: Gerber/drill layer presence, BOM/CPL counts, schematic parsing,
  archive contents and SHA-256 hashes.
- Not yet measured: actual PCB manufacture, soldering, Nano fit, sensor identity,
  power under load, signal integrity, analog accuracy and environmental behavior.

Schematic ERC is not substituted by PCB DRC. The release validation file states
whether an ERC/export check was actually run. The mating Nano is represented by
two passive sockets, so ERC alone cannot establish its power or logic ratings.
