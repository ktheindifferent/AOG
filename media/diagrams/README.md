# AOG connection diagrams

The root README contains the firmware-derived pin tables and electrical notes.
These images illustrate the Rust v2 installation with two separate Arduino
sensor boards and one four-channel I²C relay board.

## Illustrated overview

Output: `aog-system-overview.png`

Generation mode: built-in `image_gen` tool. No CLI/API fallback.

Final generation prompt:

```text
Use case: scientific-educational
Asset type: Illustrated AOG Rust v2 system overview for a GitHub README, landscape 3:2.
Primary request: Show how the AOG algae reactor components interconnect. Make a polished, highly legible technical illustration with subtle isometric hardware drawings inside organized panels. White background, dark navy lettering, teal green and muted blue accents, generous whitespace.
Title, exactly: "AOG • Rust v2"
Subtitle, exactly: "System connections"
Layout and connections:
Center: a Raspberry Pi 4 illustration labeled "Raspberry Pi" and "Rust v2 controller".
Top: laptop labeled "Web dashboard"; a Wi-Fi connection to the Pi labeled "HTTPS · 8443".
Left upper panel: one Arduino Nano labeled "Sensor kit". Inside this panel include three small sensor illustrations labeled "DHT11", "Analog CO2", "CCS811". Each connects only to this panel's Arduino. One USB cable connects the Arduino to the Pi.
Left lower panel: a SECOND separate Arduino Nano labeled "Overflow + pH". Inside this panel include two float switch icons labeled "Tank 1", "Tank 2", plus a pH probe connected through a small pH interface board labeled "pH interface". All three signals connect only to this second Arduino. One USB cable connects this Arduino to the Pi. Never connect the raw pH probe directly to the Arduino.
Bottom panel: a particulate sensor labeled "SDS011" connects through a small board labeled "USB adapter" to the Pi. Nearby optional camera labeled "Camera (optional)" with its own USB connection to the Pi.
Right upper: a small backlit LCD labeled "Status LCD · 0x72", connected to the Pi via an I2C branch.
Right middle: a board with EXACTLY FOUR relays labeled "4-relay board · 0x25", connected via the SAME I2C bus as the LCD. Put "I2C" near the branch joining LCD and relay board, not beside USB cables.
Right lower: four distinct controlled equipment illustrations labeled "Lights", "Air", "Filling", "Draining". Each has one separate logical control connection from the four-relay board. Show the fill and drain pumps as pumps and the air equipment as an aquarium air pump. Do not imply fixed relay-channel numbering. Put "Assign functions in Setup" beneath the four outputs.
Footer, exactly: "Connection overview • Exact pins and power details are in the README"
Constraints: Show exactly two Arduino boards and exactly four relay outputs. Use short, spelled-correctly labels. All connection paths must be clean, traceable, and terminate on the described modules. Connections illustrate logical data/control paths, not mains wiring. Do not draw terminal-level wiring, mains outlets, live conductors, detailed power supplies, invented electrical pins, decorative pseudo-text, or unrelated components. The illustration must remain useful at README size.
```

Final refinement prompt (built-in image edit, using the generated overview):

```text
Edit this AOG connection illustration. Preserve its title, labels, colors, layout, all panels and hardware, except for these specific connection corrections:
1. Draw FOUR distinct, continuous control paths from four separate points along the bottom of the relay board, one each to Lights, Air, Filling, and Draining. The Lights and Air paths MUST NOT join each other, and each of the four paths must clearly terminate at the top of its own equipment icon. Route around the "Controlled equipment" heading, without crossing text. The four paths must not cross or share any segment.
2. Extend the teal connection from the pH interface board so it makes visible contact with the overflow/pH Arduino board; remove the current gap below the Arduino. The pH probe still connects only through the pH interface board.
3. The blue USB path from the SDS011 USB adapter currently ends in blank space below the Raspberry Pi. Extend this path all the way to a free USB port on the Raspberry Pi, with no gap. Keep it separate from the camera USB path.
4. Replace only the optional camera icon (currently a CSI ribbon-style Pi camera board) with a compact USB webcam icon. Keep "Camera (optional)" below it and the separate blue USB connection to the Pi.
All other text and elements remain unchanged. This is a logical connection overview, so do not add pin numbers, voltage labels, mains power wiring, or extra components.
```

## Exact pin map

Outputs: `aog-pin-map.svg` (editable vector source and image) and
`aog-pin-map.png` (README preview, rendered from the SVG with CairoSVG).

The pin map is authored directly as SVG to keep all pin names and connections
exact. It is a signal map, not a scale drawing of connector positions. Its
source of truth is the Arduino firmware linked in the root README.

To regenerate the PNG with CairoSVG installed, run from the repository root:

```sh
cairosvg media/diagrams/aog-pin-map.svg -o media/diagrams/aog-pin-map.png
```
