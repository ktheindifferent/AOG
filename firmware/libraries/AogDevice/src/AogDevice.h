#pragma once
#include <Arduino.h>
#include <OneWire.h>
#include <Wire.h>
#include <EEPROM.h>
#ifdef AOG_SENSOR_KIT
#include <Adafruit_CCS811.h>
#include <DHT.h>
#endif

// No heap strings or EEPROM logging. 24 one-minute records = 24 minutes.
const uint8_t AOG_CAPACITY = 24;
const int16_t AOG_MISSING = INT16_MIN;
struct AogSample {
  uint32_t sequence, uptime, epoch;
  int16_t temp, humidity, water6, water8, ph;
  uint16_t co2, tvoc, adc;
  uint8_t overflow, valid;
};
#include "AogRing.h"
class AogWater {
 public:
  OneWire bus; uint8_t rom[8] = {}; uint8_t status = 0; int16_t value = AOG_MISSING;
  bool converting = false; uint32_t started = 0;
  AogWater(uint8_t pin): bus(pin) {}
  void beginConversion() {
    const int16_t previousValue=value; const uint8_t previousStatus=status;
    value = AOG_MISSING; status = 0; converting = false;
    bus.reset_search();
    if (!bus.search(rom)) {memset(rom,0,8);return;}
    if (OneWire::crc8(rom,7) != rom[7]) {status = 2; return;}
    if (rom[0] != 0x28) {status = 3; return;}
    uint8_t other[8];
    if (bus.search(other)) {status = 5; return;} // one dedicated probe per pin
    if (!bus.reset()) {return;}
    bus.select(rom); bus.write(0xB4); // confirm externally powered, three-wire mode
    if (!bus.read_bit()) {status = 6; return;}
    if (!bus.reset()) {return;}
    bus.select(rom); bus.write(0x44,0); started = millis(); converting = true; value=previousValue;status=previousStatus;
  }
  void finishConversion() {
    if (!converting || uint32_t(millis()-started) < 800) return;
    converting = false; value = AOG_MISSING;
    if (!bus.reset()) {status = 0;return;}
    bus.select(rom); bus.write(0xBE); uint8_t scratch[9];
    for (uint8_t i=0;i<9;++i) scratch[i] = bus.read();
    if (OneWire::crc8(scratch,8) != scratch[8]) {status = 2; return;}
    int16_t raw = (int16_t)((uint16_t(scratch[1])<<8) | scratch[0]);
    // Convert DS18B20 sixteenths to centidegrees; publish only after conversion.
    int32_t centi = int32_t(raw)*100/16;
    if (centi < -5500 || centi > 12500) {status = 4;return;}
    value = centi; status = 1;
  }
  void describe(const __FlashStringHelper* prefix) {
    Serial.print(prefix);
    switch (status) {
      case 1: Serial.println(F("ok")); break;
      case 2: Serial.println(F("crc_error")); break;
      case 3: Serial.println(F("unsupported_family")); break;
      case 4: Serial.println(F("out_of_range")); break;
      case 5: Serial.println(F("multiple_probes")); break;
      case 6: Serial.println(F("external_power_required")); break;
      default: Serial.println(F("not_detected"));
    }
  }
  void identity(const __FlashStringHelper* prefix) {
    Serial.print(prefix);
    for (uint8_t i=0;i<8;++i) {if(rom[i]<16) Serial.print('0');Serial.print(rom[i],HEX);}
    Serial.println();
  }
};

class AogDevice {
  AogRing<AogSample,AOG_CAPACITY> queue;
  AogWater water6{6}, water8{8};
  AogSample current = {};
  char boot[17] = "0000000000000000", command[64] = {};
  uint8_t commandLength = 0; bool commandOverflow = false, draining = false, held = false;
  uint32_t seq = 0, seconds = 0, clockMillis = 0, epochOffset = 0;
  uint32_t lastSample = 0, lastRead = 0, lastReport = 0;
#ifdef AOG_SENSOR_KIT
  Adafruit_CCS811 ccs;
  DHT dht{7,DHT11}; bool ccsReady = false; uint32_t ccsRetry = 0;
#endif
  uint32_t uptime() {
    uint32_t elapsed = uint32_t(millis()-clockMillis);
    seconds += elapsed/1000; clockMillis += (elapsed/1000)*1000;
    return seconds;
  }
  static bool hexId(const char* s) {
    if (!s || strlen(s)!=16 || strcmp(s,"0000000000000000")==0) return false;
    for(uint8_t i=0;i<16;++i) if(!isxdigit(s[i])) return false;
    return true;
  }
  void acceptCommand() {
    char* op = strtok(command," "); char* id = strtok(nullptr," "); char* arg = strtok(nullptr," ");
    if (!op) return;
    if (!strcmp(op,"AOG_HOLD")) {held=true;return;}
    if (!strcmp(op,"AOG_RESUME")) {held=false;return;}
    if (!strcmp(op,"AOG_DRAIN")) {draining = true; return;}
    if (!hexId(id) || !arg || !*arg || strtok(nullptr," ")) return;
    for(char* p=arg;*p;++p) if(!isdigit(*p)) return;
    unsigned long n = strtoul(arg,nullptr,10);
    if (!strcmp(op,"AOG_SYNC") && n>=1577836800UL && n<=4102444800UL) {
      if (!strcmp(boot,"0000000000000000")) strcpy(boot,id);
      if (!strcmp(id,boot)) epochOffset = n-uptime();
    } else if (!strcmp(op,"AOG_ACK") && !strcmp(id,boot)) queue.acknowledge(n);
  }
  void commands() {
    while (Serial.available()) {
      char c=Serial.read();
      if(c=='\r') continue;
      if(c=='\n') {
        if(!commandOverflow) {command[commandLength]=0; acceptCommand();}
        commandLength=0; commandOverflow=false;
      } else if(commandLength<sizeof(command)-1 && !commandOverflow) command[commandLength++]=c;
      else commandOverflow=true;
    }
  }
  static void centi(const __FlashStringHelper* name, int16_t value, char unit=0) {
    if(value==AOG_MISSING) return;
    Serial.print(name);Serial.print(value/100.0,2);if(unit)Serial.print(unit);Serial.println();
  }
  void readSensors() {
    current = {}; current.temp=current.humidity=current.ph=AOG_MISSING;
    current.water6=water6.value;current.water8=water8.value;
#ifdef AOG_SENSOR_KIT
    float t=dht.readTemperature(),h=dht.readHumidity();
    if(isfinite(t) && t>=-40 && t<=85) current.temp=t*100;
    if(isfinite(h) && h>=0 && h<=100) current.humidity=h*100;
    current.adc=analogRead(A0);
    if(!ccsReady && (ccsRetry==0 || uint32_t(millis()-ccsRetry)>30000)) {
      ccsRetry=millis(); ccsReady=ccs.begin(0x5A);
    }
    if(ccsReady) {
      if(current.temp!=AOG_MISSING && current.humidity!=AOG_MISSING) ccs.setEnvironmentalData(h,t);
      if(ccs.available()) {
        if(ccs.readData()==0) {current.co2=ccs.geteCO2();current.tvoc=ccs.getTVOC();current.valid|=1;}
        else ccsReady=false;
      }
    }
#else
    current.overflow=(digitalRead(2)==HIGH?1:0)|(digitalRead(4)==HIGH?2:0);
    uint32_t total=0;for(uint8_t i=0;i<16;++i) total+=analogRead(A0);
    current.adc=total/16;
    // Atlas Gravity nominal transfer curve. Pi calibration remains explicit;
    // ADC rails are an error, while an in-range voltage cannot prove connection.
    float mv=total/16.0*5000.0/1024.0;
    float ph=7.0-3.0*(mv-1500.0)/(mv>1500.0?530.0:525.0);
    if(current.adc>1 && current.adc<1022 && ph>=0 && ph<=14) current.ph=ph*100;
#endif
    water6.beginConversion();water8.beginConversion();
  }
  void report(const AogSample& s, bool historical) {
    Serial.println(F("BEGIN"));
#ifdef AOG_SENSOR_KIT
    Serial.println(F("DEVICE_ID: SENSORKIT_MK1"));
#else
    Serial.println(F("DEVICE_ID: DUAL_OVF_SENSOR"));
#endif
    Serial.println(F("PROTOCOL_VERSION: 2\nFIRMWARE_VERSION: 2.0.0"));
    Serial.print(F("FRAME: ")); Serial.println(historical?F("LOG"):F("LIVE"));
    Serial.print(F("BOOT_ID: "));Serial.println(boot);
    Serial.print(F("UPTIME_S: "));Serial.println(uptime());
    if(historical) {
      Serial.print(F("SEQ: "));Serial.println(s.sequence);
      Serial.print(F("SAMPLE_UPTIME_S: "));Serial.println(s.uptime);
      Serial.print(F("TIMESTAMP: "));Serial.println(s.epoch);
    } else {
      Serial.print(F("BUFFER_PENDING: "));Serial.println(queue.count);
      Serial.print(F("BUFFER_CAPACITY: "));Serial.println(AOG_CAPACITY);
      Serial.print(F("BUFFER_DROPPED: "));Serial.println(queue.dropped);
      Serial.println(F("SAMPLE_INTERVAL_S: 60"));
      Serial.print(F("LOGGING: "));Serial.println(held?F("held"):F("recording"));
      water6.describe(F("WATER_D6_STATUS: "));water8.describe(F("WATER_D8_STATUS: "));
      water6.identity(F("WATER_D6_ROM: "));water8.identity(F("WATER_D8_ROM: "));
    }
    centi(F("WATER_D6: "),s.water6,'C');centi(F("WATER_D8: "),s.water8,'C');
#ifdef AOG_SENSOR_KIT
    Serial.print(F("CCS811_STATUS: "));Serial.println((s.valid&1)?F("ok"):F("unavailable"));
    Serial.print(F("DHT_STATUS: "));Serial.println(s.temp!=AOG_MISSING && s.humidity!=AOG_MISSING?F("ok"):F("read_error"));
    if(s.valid&1) {Serial.print(F("CO2: "));Serial.print(s.co2);Serial.println(F("ppm"));Serial.print(F("TVOC: "));Serial.print(s.tvoc);Serial.println(F("ppb"));}
    Serial.print(F("CO2_ADC: "));Serial.println(s.adc);
    // A0 is preserved as raw data: its module/calibration is not identified.
    centi(F("TEMP: "),s.temp,'C');centi(F("HUM: "),s.humidity,'%');
#else
    Serial.print(F("T1_OVF: "));Serial.println(s.overflow&1?F("OVERFLOW"):F("NONE"));
    Serial.print(F("T2_OVF: "));Serial.println(s.overflow&2?F("OVERFLOW"):F("NONE"));
    Serial.print(F("PH_ADC: "));Serial.println(s.adc);
    Serial.print(F("PH_MV: "));Serial.println(s.adc*5000.0/1024.0,1);
    Serial.print(F("PH_STATUS: "));Serial.println(s.ph==AOG_MISSING?F("out_of_range"):F("analog_unverified"));
    centi(F("PH: "),s.ph);
#endif
    Serial.println(F("END"));
  }
 public:
  void begin() {
    Serial.begin(9600); Wire.begin(); Wire.setWireTimeout(25000,true);
#ifdef AOG_SENSOR_KIT
    dht.begin();
#else
    pinMode(2,INPUT_PULLUP);pinMode(4,INPUT_PULLUP);
#endif
    water6.beginConversion();water8.beginConversion();
  }
  void tick() {
    uptime();commands();water6.finishConversion();water8.finishConversion();
    if(uint32_t(millis()-lastRead)>=2000) {lastRead=millis();readSensors();}
    if(lastRead && uint32_t(millis()-lastReport)>=2000) {
      lastReport=millis();
      if(!held && (seq==0 || uint32_t(uptime()-lastSample)>=60)) {
        lastSample=uptime();current.sequence=++seq;current.uptime=lastSample;
        current.epoch=epochOffset?epochOffset+lastSample:0;queue.push(current);
      }
      report(current,false);commands();
      if(draining && strcmp(boot,"0000000000000000") && queue.first()) report(*queue.first(),true);
    }
  }
};
