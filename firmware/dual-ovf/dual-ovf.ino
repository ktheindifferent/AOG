// AOG Dual OVF: closed-to-GND safe switches D2/D4, Atlas pH A0, water D6/D8.
#include <AogDevice.h>
AogDevice device;
void setup() {device.begin();}
void loop() {device.tick();}
