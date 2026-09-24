// AOG Sensor Kit: DHT11 D7, CCS811 I2C, analog gas A0, water D6/D8.
#define AOG_SENSOR_KIT
#include <AogDevice.h>
AogDevice device;
void setup() {device.begin();}
void loop() {device.tick();}
