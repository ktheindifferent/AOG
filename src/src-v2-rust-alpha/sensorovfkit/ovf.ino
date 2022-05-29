// Copyright (c) 2020-2021 Caleb Mitchell Smith (PixelCoda)
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.


#include <Wire.h>



#define T1_OVF  2
#define T2_OVF  4



void setup(void)
{
    pinMode(T1_OVF, INPUT_PULLUP);
    pinMode(T2_OVF, INPUT_PULLUP);

    Serial.begin(74880);
    /*Wait for the chip to be initialized completely, and then exit*/

}
void loop() {
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



    delay(200);
}
