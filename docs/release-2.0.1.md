# AOG 2.0.1

Connections now provides selectable Arduino cards, separate USB and sensor health, sensor-specific wiring traces, raw reports, buffer status, and a CO₂ source comparison on desktop and mobile.

The connection inspector places each sensor’s reading, labeled wiring and raw fields together. On phones it opens beneath the selected sensor; desktop keeps the sensor list and inspector side by side. A Needs attention filter isolates reported sensor warnings, with optional or unverified connections still available in All. Live polling preserves keyboard focus, and board details, CO₂ settings, water-probe setup and firmware tools expand when needed. Failed temperature readings stay out of the live display while their raw fields remain available for diagnosis.

The DFRobot SEN0219 v1 analog infrared profile converts A0 counts into ppm using its 0.4–2.0 V / 0–5,000 ppm transfer curve. At a nominal 5 V ADC reference, 160 counts is 781.25 mV and approximately 1,191 ppm. Select the identified module, reference voltage and primary CO₂ source in Connections. New installations default to an unidentified analog input and CCS811 eCO₂; unknown modules are never assigned an arbitrary ppm conversion. SEN0219 v2 PWM is not supported by the analog profile.

The primary CO₂ value uses one selected source, with no averaging or silent fallback. Live values, history and reports use the same conversion. Infrared history/statistics exclude CCS811/legacy readings; CSV and JSON retain the originals and add source labels. Source/reference changes never reinterpret already absorbed Arduino records. Invalid and stale analog values are unavailable rather than replaced by eCO₂.

The controller version is 2.0.1. Both Arduino firmware packages remain 2.0.0: their existing raw ADC frames and RAM logs contain the required data, so this update does not require another flash. Existing configuration and history remain in place. A new `co2_settings.json` stores the optional module profile and source selection.

One existing photo-cycle test now checks explicit hours instead of assuming it runs during daytime; runtime scheduling behavior is unchanged.
