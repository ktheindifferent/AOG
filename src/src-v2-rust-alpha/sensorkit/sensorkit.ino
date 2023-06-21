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
#include <CCS811.h>
#include <dht.h>

dht DHT;
#define DHT11_PIN 7

#define T1_OVF  2
#define T2_OVF  4

CCS811 sensor;

int analogCO2SensorIn = A0;
int countCO2SensorsReporting = 0;

void setup(void)
{
 
    Serial.begin(74880);
    /*Wait for the chip to be initialized completely, and then exit*/
    while(sensor.begin() != 0){
        delay(1000);
    }

    int chk = DHT.read11(DHT11_PIN);
    sensor.setInTempHum(DHT.temperature, DHT.humidity);
    /* sensor.softReset(); */
    /* sensor.setInTempHum() */


    /**
     * @brief Set measurement cycle
     * @param cycle:in typedef enum{
     *                  eClosed,      //Idle (Measurements are disabled in this mode)
     *                  eCycle_1s,    //Constant power mode, IAQ measurement every second
     *                  eCycle_10s,   //Pulse heating mode IAQ measurement every 10 seconds
     *                  eCycle_60s,   //Low power pulse heating mode IAQ measurement every 60 seconds
     *                  eCycle_250ms  //Constant power mode, sensor measurement every 250ms
     *                  }eCycle_t;
     */
    sensor.setMeasCycle(sensor.eCycle_250ms);

    analogReference(DEFAULT);
}

float co2Average = 0.0;

void loop() {

    Serial.println("DEVICE_ID: SENSORKIT_MK1");
    Serial.println("FIRMWARE_VERSION: 001");

    float totalCO2 = 0.00;
    countCO2SensorsReporting = 0;

    if(sensor.checkDataReady() == true){
        float co2 = sensor.getCO2PPM();
        Serial.print("TVOC: ");
        Serial.print(sensor.getTVOCPPB());
        Serial.println("ppb");
        countCO2SensorsReporting = countCO2SensorsReporting + 1;
        totalCO2 = totalCO2 + co2;
    } 
    sensor.writeBaseLine(0x847B);
    //delay(500);

    //Read voltage
    int analogCO2SensorValue = analogRead(analogCO2SensorIn);

    // The analog signal is converted to a voltage
    float voltage = analogCO2SensorValue*(5000/1024.0);
    if(voltage > 400){
      int voltage_diference=voltage-400;
      float concentration=voltage_diference*50.0/16.0;
      countCO2SensorsReporting = countCO2SensorsReporting + 1;
      totalCO2 = totalCO2 + concentration;
    }

    if(co2Average < 1.0){
        co2Average = totalCO2/countCO2SensorsReporting;
    } else {
        co2Average = ((co2Average + (totalCO2/countCO2SensorsReporting))/2);
    }


    Serial.print("CO2: ");
    Serial.print(co2Average);
    Serial.println("ppm");

    int chk = DHT.read11(DHT11_PIN);
    Serial.print("HUM: ");
    Serial.print(DHT.humidity);
    Serial.println("%");
    Serial.print("TEMP: ");
    Serial.print(DHT.temperature);
    Serial.println("C");


    //delay(200);//Wait 1 seconds before accessing sensor again.
}
