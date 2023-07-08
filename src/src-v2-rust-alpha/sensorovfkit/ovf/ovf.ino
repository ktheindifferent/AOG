// to use the Atlas gravity circuits with
// the gravity isolator board's pulse output
// uncomment line 8: #define USE_PULSE_OUT
// you can use any pins instead of just the analog ones
// but it must be recalibrated
// note that the isolator's analog output also provides isolation

// #define USE_PULSE_OUT

#include <Wire.h>



#define T1_OVF  2
#define T2_OVF  4


#ifdef USE_PULSE_OUT
#include "ph_iso_grav.h"
Gravity_pH_Isolated pH = Gravity_pH_Isolated(A0);
#else
#include "ph_grav.h"
Gravity_pH pH = Gravity_pH(A0);
#endif


uint8_t user_bytes_received = 0;
const uint8_t bufferlen = 32;
char user_data[bufferlen];

void parse_cmd(char* string) {
  strupr(string);
  if (strcmp(string, "CAL,7") == 0) {
    pH.cal_mid();
  }
  else if (strcmp(string, "CAL,4") == 0) {
    pH.cal_low();
  }
  else if (strcmp(string, "CAL,10") == 0) {
    pH.cal_high();
  }
  else if (strcmp(string, "CAL,CLEAR") == 0) {
    pH.cal_clear();
  }
}

void setup() {
  pinMode(T1_OVF, INPUT_PULLUP);
  pinMode(T2_OVF, INPUT_PULLUP);

  Serial.begin(9600);
  pH.begin();
}

void loop() {

  float phf = pH.read_ph();
  String mmts = "";     // empty string
  mmts.concat(phf);
  
  Serial.println("BEGIN");
  Serial.println("DEVICE_ID: DUAL_OVF_SENSOR");
  Serial.println("FIRMWARE_VERSION: 001");
  Serial.println("P1: PIN_2");
  Serial.println("P2: PIN_4");



  int val = digitalRead(T1_OVF);  // read input value
  if (val == LOW) {         // check if the input is HIGH (button released)
    Serial.println("T1_OVF: NONE");
  } else {
    Serial.println("T1_OVF: OVERFLOW");
  }



  int val2 = digitalRead(T2_OVF);  // read input value
  if (val2 == LOW) {         // check if the input is HIGH (button released)
    Serial.println("T2_OVF: NONE");
  } else {
    Serial.println("T2_OVF: OVERFLOW");
  }

  
  Serial.println("PH: " + mmts);
  Serial.println("END");
  delay(1000);
}
