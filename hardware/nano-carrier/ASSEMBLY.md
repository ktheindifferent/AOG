# AOG Nano Carrier Rev A — assembly and bring-up

## Factory assembly

Populate **every component in bom.csv** on the component/top side. Use the
manufacturer part numbers, not just a value or matching package name.
J1/J2 are **female 15-way sockets**, 8.5 mm high, with 0.64 mm square mating
contacts. The Nano plugs into them later and must not be soldered to the PCB.
Keep the two sockets upright and parallel during soldering; use a suitable
unpowered mating fixture to preserve the 15.24 mm row spacing.

J3–J10 are 3.81 mm pitch screw terminals. Their connection openings face out:
J3/J4/J7/J8 left, J5/J6/J9 right, J10 bottom. Confirm orientation against the
part drawing and assembly.pdf. Do not substitute 3.5 mm or 5.08 mm terminals.
METZ CONNECT ordering numbers are 31086102 / 31086103 / 31086104, corresponding
to RT03402HBLC / RT03403HBLC / RT03404HBLC. Any substitution needs a matching
pin diameter, body outline, pitch and wire-entry orientation.

U1 is Microchip **MCP1700T-3302E/TT** (SOT-23): 1=GND, 2=VOUT, 3=VIN.
Q1/Q2 are **BSS138-7-F**: 1=gate, 2=source, 3=drain. Other SOT-23 regulators
or MOSFETs are not automatically pin compatible. D1 pad 1 is cathode/GND;
pad 2 is anode/LED_A. Capacitors are non-polarized ceramics; retain the
specified voltage and dielectric. U1's output capacitor must retain at least
1 µF effective capacitance at 3.3 V.

In the supplied top-view, 0° SOT-23 orientation, pad 1 is **upper left**, pad 2
is **lower left**, and pad 3 is **right**. D1 cathode/pad 1 is on the left.
Terminal plated holes are **1.1 mm finished diameter**, following the METZ
CONNECT 2025-08-21 drawing (0.9 × 0.5 mm solder tails). The project includes
corrected local footprints; do not replace them with the older KiCad footprint
whose 0.7 mm holes are too small for this part.

TP1–TP4 are exposed copper test pads, not purchased parts. H1–H4 are unplated
3.2 mm mounting holes; use insulating standoffs or respect the copper keep-out.
No parts are fitted beneath the Nano outline. Its USB connector points to the
top edge and remains accessible. Allow space above the carrier for the socket,
Nano and cable; the carrier alone does not define a complete enclosure.

## Position-file convention

Millimetres. Origin is the **bottom-left corner** of the finished 80 × 80 mm
PCB, viewed from the top; X increases right, Y increases up. The Gerber and
Excellon files use that same origin. Rotation is KiCad's component rotation,
counter-clockwise viewed from the top. All placements are on Top.

SMD coordinates use the footprint placement origin. THT coordinates are the
midpoint of the pin row and are supplied for manual/assisted placement;
the terminal body is not necessarily centered on that row. Use the assembly
drawing for orientation rather than assuming a supplier library's 0° angle
matches KiCad. In particular, confirm U1/Q1/Q2 and D1 in the factory preview.

## Unpowered inspection and electrical checks

1. Inspect solder joints and polarity, especially U1/Q1/Q2 and the Nano rows.
   Verify the finished board is 80 × 80 mm and socket centers are 15.24 mm
   apart. Check the actual Nano against the 1:1 PDF before applying power.
2. With no Nano or sensors inserted, confirm no short between 5 V and GND or
   3.3 V and GND. Capacitors may briefly charge during a resistance test.
3. Verify J3 data to 5 V and J4 data to 5 V each measure approximately 4.7 kΩ.
   Verify J7/J8 signal to 5 V and J6 data to 5 V likewise. Verify each I²C
   pull-up against the schematic; the regulator/FETs may affect in-circuit
   readings, so investigate unexpected results rather than treating this as
   a four-terminal resistor measurement.
4. Verify continuity from terminal data to the intended socket pad, using
   connectivity.csv. J5 signal goes through R5 (1 kΩ) before reaching A0.
   The two water data lines must not be shorted together.

## Factory power test with the Nano absent

For a factory fixture only, apply a **current-limited regulated 5.0 V** supply
to TP1 (+5V) and TP2 (GND), with no Nano, USB connection or sensors present.
Start with a 30 mA limit. Check for unexpected current or heating; D1 should
light and TP3 should be approximately 3.3 V. A conservative room-temperature
acceptance range is 3.20–3.40 V, unloaded. TP4/A0 should settle near ground.
Remove fixture power before inserting the Nano. **This fixture test is not
permission to connect external 5 V while a Nano's USB is connected.**

Factory continuity, power and visual checks can be requested as part of the
quote; they are not claimed to have been performed on an unbuilt prototype.

## On-system prototype bring-up

1. Keep controlled pumps disabled while qualifying the new hardware. Disconnect
   USB and other power, insert the **classic 5 V Nano** with USB at the marked
   top edge, and ensure every male pin enters its matching socket. Connect only
   USB initially. Do not insert a Nano backwards or offset by one position.
2. Verify board identity and the existing firmware in AOG Connections. A power
   LED proves only rail power, not sensor health. This carrier does not need a
   new firmware version or flash to preserve the installed pin assignments.
3. Measure the 5 V rail under expected sensor load. Its actual voltage follows
   the Nano's USB power path; it is not regulated to exactly 5.000 V by this
   carrier. Verify the analog CO₂ module's specified supply range and enter
   the measured ADC reference in AOG. Resolve undervoltage before calibration.
4. Disconnect power between wiring changes. Identify the water probes' model
   and VDD/DQ/GND wires, then test **one confirmed externally powered DS18B20**
   on J3. Check ROM family 28, CRC/temperature status and plausible temperature
   against a reference thermometer. Repeat on J4. No external pull-up is needed
   when the fitted R1/R2 are present. Do not add a second pull-up unnecessarily.
5. On Dual OVF, test J7 and J8 separately with the actual dry contacts. Open must
   show overflow/unsafe; closed to GND must show safe. Verify the associated
   pump interlocks before allowing normal operation. The filter does not
   distinguish a broken cable from an open overflow switch.
6. On Dual OVF J5, connect Gravity `+` to 5 V, `A` to signal, `−` to GND. Retain
   the electrode on its interface board. Confirm the physical connection in
   AOG, check known pH buffers and recalibrate. An unplugged electrode cannot
   be diagnosed from an in-range analog voltage.
7. On Sensor Kit, verify the DHT11 on J6 and the identified analog v1 CO₂ module
   on J5. Observe the CO₂ module's warm-up and calibration requirements.
   CCS811 eCO₂ and infrared CO₂ remain separate data sources.
8. Before connecting J9, identify the CCS811 breakout's power and pin labels.
   Confirm that it accepts 3.3 V and uses address 0x5A. Connect SDA/SCL/GND by
   name. If it exposes WAKE, hold WAKE low using J10; if ADDR must be strapped
   low for 0x5A, use J10's other ground terminal. Do not assume its header order
   matches this carrier. Verify I²C reporting and low/high levels; scope the
   bus if pull-ups/cable capacitance cause intermittent communication.
9. Check both boards through a reconnect, observe fresh values and several
   history records, then restore normal operation after the interlock checks.

Remaining physical qualification: exact Nano fit/USB clearance, supply voltage
under sensor peaks, sensor identification, analog accuracy, cable/noise effects,
and operation in the installed enclosure. CAD checks do not measure these.
