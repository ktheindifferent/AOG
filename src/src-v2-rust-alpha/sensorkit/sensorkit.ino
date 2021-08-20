#include <Wire.h>
#include <CCS811.h>
#include <dht.h>

dht DHT;
#define DHT11_PIN 7

#define TOP_TANK_FLOAT  2
#define MAIN_TANK_FLOAT  4

CCS811 sensor;

int analogCO2SensorIn = A0;
int countCO2SensorsReporting = 0;

void setup(void)
{
    pinMode(TOP_TANK_FLOAT, INPUT_PULLUP);
    pinMode(MAIN_TANK_FLOAT, INPUT_PULLUP);

    Serial.begin(9600);
    /*Wait for the chip to be initialized completely, and then exit*/
    while(sensor.begin() != 0){
        Serial.println("failed to init chip, please check if the chip connection is fine");
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
void loop() {
  float totalCO2 = 0.00;
  countCO2SensorsReporting = 0;
  // delay(2000);
    if(sensor.checkDataReady() == true){
        float co2 = sensor.getCO2PPM();
        Serial.print("S1CO2: ");
        Serial.print(co2);
        Serial.print("ppm\nS1TVOC: ");
        Serial.print(sensor.getTVOCPPB());
        Serial.println("ppb");
        countCO2SensorsReporting = countCO2SensorsReporting + 1;
        totalCO2 = totalCO2 + co2;
    } else {
        Serial.println("Data is not ready!");
    }
    /*!
     * @brief Set baseline
     * @param get from getBaseline.ino
     */
    //sensor.writeBaseLine(0x847B);
    //delay cannot be less than measurement cycle
    //delay(1000);

   //Read voltage
    int analogCO2SensorValue = analogRead(analogCO2SensorIn);

    // The analog signal is converted to a voltage
    float voltage = analogCO2SensorValue*(5000/1024.0);
    if(voltage == 0)
    {
      Serial.println("Fault");
    }
    else if(voltage < 400)
    {
      Serial.println("preheating");
    }
    else
    {
      int voltage_diference=voltage-400;
      float concentration=voltage_diference*50.0/16.0;
      Serial.print("S2CO2: ");
      Serial.print(concentration);
      Serial.println("ppm");
      countCO2SensorsReporting = countCO2SensorsReporting + 1;
      totalCO2 = totalCO2 + concentration;
    }

    Serial.print("AVGCO2: ");
    Serial.print(totalCO2/countCO2SensorsReporting);
    Serial.println("ppm");

    int chk = DHT.read11(DHT11_PIN);
    Serial.print("HUMIDITY: ");
    Serial.print(DHT.humidity);
    Serial.print("%\n");
    Serial.print("TEMPERATURE: ");
    Serial.print(DHT.temperature);
    Serial.println("C  ");

    int val = digitalRead(TOP_TANK_FLOAT);  // read input value
    if (val == LOW) {         // check if the input is HIGH (button released)
      Serial.println("TOP_TANK_OVERFLOW: NONE");
    } else {
      Serial.println("TOP_TANK_OVERFLOW: OVERFLOW");
    }

    int val2 = digitalRead(MAIN_TANK_FLOAT);  // read input value
    if (val2 == LOW) {         // check if the input is HIGH (button released)
      Serial.println("BARREL_WATER_OVERFLOW: NONE");
    } else {
      Serial.println("BARREL_WATER_OVERFLOW: OVERFLOW");
    }

    // delay(2000);//Wait 1 seconds before accessing sensor again.
}
