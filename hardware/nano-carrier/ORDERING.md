# Order two populated AOG Nano carriers

Use **one design, quantity two assembled boards**. Both are fully populated
with the same BOM; you insert your existing Sensor Kit and Dual OVF Nanos
after delivery. This avoids two separate assembly setups. A supplier may
fabricate a minimum of five bare boards and assemble only two.

## Recommended first quote: PCBWay

[PCBWay's assembly service](https://www.pcbway.com/pcb-assembly.html) supports
mixed surface-mount and through-hole assembly, and its
[minimum-quantity FAQ](https://www.pcbway.com/helpcenter/pcb_assembly_ordering/What_s_the_min_quantity_of_assembly_.html)
states a one-piece assembly minimum. This is a practical starting point for
the Nano sockets and screw terminals, as well as the SMD parts.

1. Download and unzip `rev-a/AOG-Nano-Carrier-Rev-A-Assembly-Package.zip`.
   The extracted `rev-a` folder contains the files to upload; the guides are
   next to that folder.
2. Open the [PCB assembly quote form](https://www.pcbway.com/quotesmt.aspx).
   Select **turnkey** (supplier sources the BOM), **2 assembled boards**,
   **top-side placement**, and **mixed SMT + through-hole assembly**.
3. Upload `AOG-Nano-Carrier-Rev-A-Gerbers.zip` in the fabrication/Gerber field,
   `bom.csv` as the BOM, and `positions.csv` as the centroid/placement file.
   Attach `assembly.pdf` and `ASSEMBLY.md` as assembly instructions. The
   full assembly-package ZIP is for an engineer's review; do not upload it
   as though it were a Gerber-only ZIP.
4. Use **80 × 80 mm, 2 layers, FR-4, 1.6 mm thick, 1 oz copper, green solder
   mask, white silkscreen, lead-free HASL**, ordinary prototype tolerances.
   ENIG is optional and not required by these footprints. Let the assembler
   add sacrificial panel rails, tooling holes and fiducials outside the board
   if its process needs them. Ship depanelized individual boards.
5. Ask the supplier to **populate the female sockets and all screw terminals**.
   Review the component placement/polarity preview and the parts substitutions
   before paying. Nano modules and sensors are customer supplied after delivery;
   the assembler does not program or solder a Nano onto the carrier.
6. Compare the **complete landed price**, including PCB minimum quantity,
   components, component procurement, SMT setup/stencil, through-hole labor,
   electrical testing, shipping and any destination taxes/import charges.

If the form requests counts: the standard BOM has **37 placements**:
**27 SMD components and 10 through-hole components**, representing **14
unique manufacturer part numbers**. There are **57 SMD component pads and
52 component through holes**. Test pads, mounting holes and vias are not
component placements. The included BOM is authoritative if revised.

Paste this in the quote notes:

> Please quote TWO identical fully populated AOG Nano Carrier Rev A PCBs,
> 80 × 80 mm, 2-layer FR-4, 1.6 mm, 1 oz copper, green/white, lead-free HASL.
> Turnkey procurement and mixed top-side SMT plus through-hole assembly.
> Include all BOM resistors, capacitors, regulator, MOSFETs, LED, TWO FEMALE
> 1×15 Nano sockets per board, and all screw terminals. Customer inserts a
> classic 5 V ATmega328P Nano and connects external sensors after delivery.
> No Nano, sensor, firmware programming, or assembled cable harness is in
> this BOM. Supply depanelized boards. Check terminal wire-entry orientation
> and U1/Q1/Q2/LED polarity against assembly.pdf. Gerber, drill and centroid
> coordinates share the bottom-left board origin, millimetres, X right/Y up.
> Please confirm the full delivered price and obtain approval before
> substituting a connector or changing a footprint. This is a first prototype.

## Compare with JLCPCB

[JLCPCB prototype assembly](https://jlcpcb.com/solutions/pcb-prototype-assembly)
advertises a two-board assembly minimum, and its
[assembly capabilities](https://jlcpcb.com/capabilities/pcb-assembly-capabilities)
include through-hole and mixed assembly. Upload the same Gerber ZIP and use
the supplied `bom-jlc.csv` and `positions-jlc.csv` if its importer requests
the JLC column names. Search/match the **exact manufacturer part numbers**;
LCSC stock numbers have deliberately not been guessed.

For these named Nano sockets and terminals, confirm sourcing and through-hole
installation. Some parts may require Standard PCBA or a special procurement
quote. A quote for SMT parts alone would leave soldering for you and does not
meet the requested ready-to-use assembly. Check the component preview because
supplier libraries can use different zero-degree rotations.

## What the low advertised PCB prices mean

Prices checked on **2026-09-25**, not a binding quote:

| Service | Published entry offer | What it means for this project |
| --- | --- | --- |
| [JLCPCB](https://jlcpcb.com/ads/pcb-manufacturer-comparison?style=v2) | From $2 for 5 standard boards up to 100 × 100 mm | Bare PCB promotion; components, assembly and shipping are extra, and eligibility varies |
| [PCBWay](https://m.pcbway.com/activitypcb.aspx) | $5 for 10 1–2 layer boards up to 100 × 100 mm | Bare PCB promotion, not the delivered assembled-board price |
| [OSH Park](https://docs.oshpark.com/services/two-layer/) | $5/in², including three copies | At 80 × 80 mm, about $49.60 for three bare boards; you still need an assembly service |

For your **factory-assembled** requirement, compare PCBWay and JLCPCB's final
two-board quotes. Ordering two identical carriers shares setup costs, but
there is no verified delivered price until the supplier prices this BOM and
your shipping address. No paid order or external quote request was submitted.

The carrier design has CAD validation; there is no built prototype yet.
Order a small prototype batch and complete the bring-up in `ASSEMBLY.md`
before attaching it to active pump control. This is also where the unknown
water-probe model and CCS811 breakout compatibility are checked.
