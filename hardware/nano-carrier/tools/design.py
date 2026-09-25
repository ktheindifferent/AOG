"""AOG Nano carrier Rev A electrical definition (top/component-side coordinates).

J1/J2 are the two actual female sockets, not an Arduino supplied by the assembler.
USB points toward the top edge. All dimensions are millimetres.
"""
from dataclasses import dataclass

WIDTH = HEIGHT = 80.0
REVISION = "A"
NAME = "aog-nano-carrier-rev-a"

@dataclass
class Part:
    ref: str
    value: str
    kind: str
    x: float
    y: float
    angle: float
    nets: list
    mpn: str
    maker: str
    purpose: str

PARTS = []
def add(ref, value, kind, x, y, nets, mpn, maker, purpose, angle=0):
    PARTS.append(Part(ref,value,kind,x,y,angle,nets,mpn,maker,purpose))

# Official classic Nano, TOP VIEW, USB toward the top edge of this carrier.
# Socket pin 1 is at the USB end on BOTH rows. These are socket pin numbers,
# not the 30-pin Arduino module's counterclockwise package numbering.
LEFT = ['NC_D13','NC_3V3','NC_AREF','A0','NC_A1','NC_A2','NC_A3',
        'SDA_5V','SCL_5V','NC_A6','NC_A7','+5V','NC_RESET_L','GND','NC_VIN']
RIGHT = ['NC_D12','NC_D11','NC_D10','NC_D9','D8','D7','D6','NC_D5',
         'D4','NC_D3','D2','GND','NC_RESET_R','NC_RX','NC_TX']
add('J1','Nano left socket','socket15',32.38,6,LEFT,'M20-7821546','Harwin','Female Nano socket, USB end is pin 1')
add('J2','Nano right socket','socket15',47.62,6,RIGHT,'M20-7821546','Harwin','Female Nano socket, USB end is pin 1')

def terminal(ref, value, x,y,nets,purpose,angle=0):
    mpn = f'RT0340{len(nets)}HBLC'
    add(ref,value,f'terminal{len(nets)}',x,y,nets,mpn,'METZ CONNECT',purpose,angle)

terminal('J3','WATER D6',6,12,['+5V','D6','GND'],'Externally powered DS18B20 only',270)
terminal('J4','WATER D8',6,32,['+5V','D8','GND'],'Externally powered DS18B20 only',270)
terminal('J5','ANALOG A0',74,20,['+5V','ANALOG_IN','GND'],'Gravity pH on Dual OVF OR analog CO2 on Sensor Kit',90)
terminal('J6','DHT11 D7',74,39,['+5V','D7','GND'],'DHT11 climate sensor, Sensor Kit role',90)
terminal('J7','OVERFLOW D2',6,52,['D2','GND'],'Dry contact closed to ground when safe',270)
terminal('J8','OVERFLOW D4',6,66,['D4','GND'],'Dry contact closed to ground when safe',270)
terminal('J9','CCS811 3V3',74,62,['+3V3','GND','SDA_3V3','SCL_3V3'],'3.3 V compatible breakout; firmware address 0x5A',90)
terminal('J10','WAKE / ADDR GND',57,73,['GND','GND'],'Optional ground leads for CCS811 WAKE and ADDR')

def resistor(ref,value,x,y,nets,purpose,angle=0):
    code={'4.7k':'4K7','1k':'1K','1M':'1M','2.2k':'2K2'}[value]
    add(ref,value,'R0805',x,y,nets,'RC0805FR-07'+code+'L','Yageo',purpose,angle)
resistor('R1','4.7k',18,20,['+5V','D6'],'D6 1-Wire pull-up')
resistor('R2','4.7k',18,40,['+5V','D8'],'D8 1-Wire pull-up')
resistor('R3','4.7k',20,55,['+5V','D2'],'Overflow 1 external pull-up')
resistor('R4','4.7k',20,69,['+5V','D4'],'Overflow 2 external pull-up')
resistor('R5','1k',60,17,['ANALOG_IN','A0'],'Analog input series filter')
resistor('R6','1M',60,23,['A0','GND'],'Weak pull-down on disconnected analog cable; not probe detection')
resistor('R7','4.7k',60,37,['+5V','D7'],'DHT11 data pull-up')
resistor('R8','4.7k',37,52,['+5V','SDA_5V'],'SDA high-side pull-up')
resistor('R9','4.7k',49,56,['+3V3','SDA_3V3'],'SDA low-side pull-up')
resistor('R10','4.7k',37,62,['+5V','SCL_5V'],'SCL high-side pull-up')
resistor('R11','4.7k',49,66,['+3V3','SCL_3V3'],'SCL low-side pull-up')
resistor('R12','2.2k',30,76,['+5V','LED_A'],'Power LED current limit')

def capacitor(ref,value,x,y,nets,purpose,angle=0):
    mpn={'100n':'GRM21BR71E104KA01L','2.2u':'GRM21BR71E225KA73L','10u':'GRM21BR61E106KA73L'}[value]
    add(ref,value,'C0805',x,y,nets,mpn,'Murata',purpose,angle)
capacitor('C1','10u',40,72,['+5V','GND'],'5 V bulk decoupling; 25 V X5R')
capacitor('C2','100n',34,72,['+5V','GND'],'Local 5 V decoupling; 25 V X7R')
capacitor('C3','100n',18,14,['+5V','GND'],'D6 connector power decoupling')
capacitor('C4','100n',18,34,['+5V','GND'],'D8 connector power decoupling')
capacitor('C5','100n',55,23,['A0','GND'],'Analog low-pass filter; nominal 1.59 kHz')
capacitor('C6','100n',60,31,['+5V','GND'],'DHT11 supply decoupling')
capacitor('C7','2.2u',54,48,['+5V','GND'],'LDO input ceramic, 25 V X7R')
capacitor('C8','2.2u',59,55,['+3V3','GND'],'LDO output ceramic, 25 V X7R; retain >=1uF effective')
capacitor('C9','100n',66,56,['+3V3','GND'],'CCS811 connector decoupling')
capacitor('C10','100n',27,55,['D2','GND'],'Overflow 1 contact filter')
capacitor('C11','100n',27,69,['D4','GND'],'Overflow 2 contact filter')

# Manufacturer SOT23 pin assignments, not interchangeable regulator pinouts.
add('U1','MCP1700T-3302E/TT','SOT23',59,49, ['GND','+3V3','+5V'],
    'MCP1700T-3302E/TT','Microchip','3.3 V regulator: pin 1 GND, 2 OUT, 3 IN')
add('Q1','BSS138-7-F','SOT23',43,53,['+3V3','SDA_3V3','SDA_5V'],
    'BSS138-7-F','Diodes Incorporated','I2C SDA: pin 1 gate, 2 source/3.3V side, 3 drain/5V side')
add('Q2','BSS138-7-F','SOT23',43,63,['+3V3','SCL_3V3','SCL_5V'],
    'BSS138-7-F','Diodes Incorporated','I2C SCL: pin 1 gate, 2 source/3.3V side, 3 drain/5V side')
add('D1','GREEN','LED0805',38,76,['GND','LED_A'],'LTST-C170KGKT','Lite-On','5 V power indication: pin 1 cathode')

TESTPOINTS = [('TP1',24,47,'+5V'),('TP2',24,44,'GND'),('TP3',63,62,'+3V3'),('TP4',55,27,'A0')]
HOLES=[(3.5,3.5),(76.5,3.5),(3.5,76.5),(76.5,76.5)]
