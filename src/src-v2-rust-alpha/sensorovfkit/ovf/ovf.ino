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

  Serial.begin(115200);
  pH.begin();
}

void loop() {
  Serial.print("BEGIN\n");
  Serial.print("DEVICE_ID: DUAL_OVF_SENSOR\n");
  Serial.print("FIRMWARE_VERSION: 001\n");
  Serial.print("P1: PIN_2\n");
  Serial.print("P2: PIN_4\n");



  int val = digitalRead(T1_OVF);  // read input value
  if (val == LOW) {         // check if the input is HIGH (button released)
    Serial.print("T1_OVF: NONE\n");
  } else {
    Serial.print("T1_OVF: OVERFLOW\n");
  }



  int val2 = digitalRead(T2_OVF);  // read input value
  if (val2 == LOW) {         // check if the input is HIGH (button released)
    Serial.print("T2_OVF: NONE\n");
  } else {
    Serial.print("T2_OVF: OVERFLOW\n");
  }



  if (Serial.available() > 0) {
    user_bytes_received = Serial.readBytesUntil(13, user_data, sizeof(user_data));
  }

  if (user_bytes_received) {
    parse_cmd(user_data);
    user_bytes_received = 0;
    memset(user_data, 0, sizeof(user_data));
  }
  Serial.print("PH: ");
  Serial.print(pH.read_ph());
  Serial.print("\n");
  Serial.print("END\n");
  delay(500);
}
