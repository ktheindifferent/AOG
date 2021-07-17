#!/usr/bin/python
# -*- coding: UTF-8 -*-

import os
import io
import csv
import sys
import _thread
import sds011
import socket
import subprocess
import picamera
import logging
import socketserver
import threading
import serial, time, struct
from http import server
from datetime import datetime
from threading import Condition
from KasaSmartPowerStrip import SmartPowerStrip

server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEPORT, 1)
server.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
server.settimeout(0.2)
broadcastMessage = b"AOG.SERVER.(IPADDR)"


TIME_KEY = time.time()
S1CO2 = 0.00
S2CO2 = 0.00
AVGCO2 = 0.00
HUMIDITY = 0.00
TEMPERATURE = 0.00
co2pre = ""
VERSION = "1.0.6"


TOTALCO2 = "N/A"
TVOC = "N/A"
PM_25 = "N/A"
PM_10 = "N/A"
HUMIDITYL = "N/A"
TEMPERATUREL = "N/A"
BARREL_WATER_LEVEL = "N/A"
OVERFLOW_STATUS = "N/A"
TOP_TANK_OVERFLOW = "N/A"

skipUpdateAccept = False
autoUpdateAccept = False

emergancyTopTankOverflowStop = False

print("Algae Oxygen Generator (A.O.G) v"+VERSION)

if len(sys.argv) > 1:
	skipUpdateAccept = True
	if sys.argv[1] == '-y':
		autoUpdateAccept = True


if skipUpdateAccept == False:
	while True:
		query = input('Do you want to check for updates?')
		Fl = query[0].lower()
		if query == '' or not Fl in ['y','n']:
			print('Please answer with yes or no!')
		else:
			break
	if Fl == 'y':
		print("Updating Firmware....")
		os.system('sudo apt-get update')
		os.system('sudo apt-get upgrade')
	if Fl == 'n':
		print("Skipping updates....")


if autoUpdateAccept == True:
	print("Updating Firmware....")
	os.system('sudo apt-get update')
	os.system('sudo apt-get upgrade')

# create DataLog.csv for this session using stored timestamp as a key
with open('/home/pi/logs/algae_datalog'+str(TIME_KEY)+'.csv', mode='w') as logfile:
	today = datetime.now()
	employee_writer = csv.writer(logfile, delimiter=',', quotechar='"', quoting=csv.QUOTE_MINIMAL)
	employee_writer.writerow(["S1CO2", "S2CO2", "AVGCO2", "TVOC", "PM2.5", "PM10", "HUMIDITY", "TEMPERATURE", "TIMESTAMP"])


# Write Data to CSV File
def logData(sensor1co2, sensor2co2, avgCo2, tvoc, pm25, pm10, hum, temp):
	with open('/home/pi/logs/algae_datalog'+str(TIME_KEY)+'.csv', mode='a') as logfile:
		today = datetime.now()
		employee_writer = csv.writer(logfile, delimiter=',', quotechar='"', quoting=csv.QUOTE_MINIMAL)
		employee_writer.writerow([sensor1co2, sensor2co2, avgCo2, TVOC, pm25, pm10, hum, temp, today])


def cyclePump():
	import RPi.GPIO as GPIO
	import time

	GPIO.setmode(GPIO.BOARD)
	GPIO.setup(11, GPIO.OUT)
	GPIO.setup(13, GPIO.OUT)
	GPIO.setup(15, GPIO.OUT)

	GPIO.output(11, 1) # switch off
	GPIO.output(13, 1) # switch off
	GPIO.output(15, 1) # switch off

	# now = datetime.now()
	# current_hour = int(now.strftime("%H"))
	# if current_hour > 0 and current_hour < 6:
	# 	GPIO.output(13, 1) # switch off
	# 	GPIO.output(15, 1) # switch off
	# if current_hour >= 6:
	# 	GPIO.output(13, 0) # switch on
	# 	# Experimental - Does hourly rest and recovery period help algae growth?
	# 	# If hour is odd then shutdown the air pump.
	# 	if (current_hour % 2) == 0:
	# 		GPIO.output(15, 0)
	# 	else:
	# 		GPIO.output(15, 1)

	# # Wait 250 seconds before pump cycle to ensure tank is drained.
	# # This prevents overflows
	# # time.sleep(250)

	# Pump Cycle
	while True:
		loop_now = datetime.now()
		loop_current_time = loop_now.strftime("%H:%M:%S")
		loop_current_hour = int(loop_now.strftime("%H"))
		print("Current Time =", loop_current_time)


		if loop_current_hour >= 0 and loop_current_hour <= 6:
			GPIO.output(13, 1) # switch off light
			GPIO.output(15, 1) # switch off air pump
			
		if loop_current_hour > 6:
			GPIO.output(13, 0) # switch on light

			# Experimental - Does hourly rest and recovery period help algae growth?
			# If hour is odd then shutdown the air pump.
			if (loop_current_hour % 2) == 0:
				GPIO.output(15, 0) # air pump on
			else:
				GPIO.output(15, 1) # air pump off


		runtime = 60
		while runtime > 0 and emergancyTopTankOverflowStop == False:
			GPIO.output(11, 0)

			# Experimental - Does hourly rest and recovery period help algae growth?
			# If hour is odd then shutdown the water pump.
			if (loop_current_hour % 2) == 0:
				GPIO.output(11, 0) #on
			else:
				GPIO.output(11, 1) #off

			time.sleep(1)
			runtime -= 1
		GPIO.output(11, 1) # switch off
		time.sleep(200)




# Start Web Server
def webServ():
	import io
	import cgi
	import picamera
	import logging
	import socketserver
	import threading
	from threading import Condition
	from http import server
	import _thread

	message = ""

	class StreamingOutput(object):
		def __init__(self):
			self.frame = None
			self.buffer = io.BytesIO()
			self.condition = Condition()

		def write(self, buf):
			if buf.startswith(b'\xff\xd8'):
				# New frame, copy the existing buffer's content and notify all
				# clients it's available
				self.buffer.truncate()
				with self.condition:
					self.frame = self.buffer.getvalue()
					self.condition.notify_all()
				self.buffer.seek(0)
			return self.buffer.write(buf)

	class StreamingHandler(server.BaseHTTPRequestHandler):
		def do_GET(self):
			PAGE="""\
			<html>
			<head>
			<title>AOG v"""+VERSION+"""</title>
			<link rel="stylesheet" href="/css/bootstrap.min.css">
			<link rel="stylesheet" href="/css/main.css">
			<script src="/js/jquery-3.5.1.min.js"></script>
			<script src="/js/bootstrap.min.js"></script>
			</head>



			<body>
			<div id='stars'></div>
			<div id='stars2'></div>
			<div id='stars3'></div>


			<span style="
				top: 40;
				right: 0;
				position: fixed;
				height: 100%;
			"><img src="stream.mjpg" height="100%"></span>
			<br/>

			<h3 style="
				color: white;
			">Algae Oxygen Generator (A.O.G.) v"""+VERSION+"""</h3>
			<p style="
				font-size: 16px;
				color: white;
			">

			<hr style="
				color: white;
				background-color: white;
			"/>

			<!-- Button trigger modal -->
			<!-- <button style="position: fixed; top: 0; right: 0;" type="button" class="btn btn-primary" data-toggle="modal" data-target="#settingsModal">
				Settings
			</button> -->
			<span style="position: fixed; top: 0; right: 0; float: right;">
			<a href="/reboot.html" class="btn btn-danger" style="color: white;">Reboot</a>
			<a href="/shutdownPump.html" class="btn btn-danger" style="color: white;">Shutdown Pump</a>
			<a href="/activatePump.html" class="btn btn-success" style="color: white;">Activate Pump</a>
			<a target="_blank" href="/stream.mjpg" class="btn btn-primary" style="color: white;">Open Fullscreen</a>
			</span>

			<strong>CO2:</strong> """+TOTALCO2+"""<br/><br/><strong>TVOC</strong>: """+TVOC+"""<br/><br/><strong>PM 2.5</strong>: """+PM_25+"""<br/><br/>
			<strong>PM 10</strong>: """+PM_10+"""<br/><br/><strong>Humidity</strong>: """+HUMIDITYL+"""
			<br/><br/><strong>Air Temperature</strong>: """+TEMPERATUREL+"""
			<br/><br/><strong>Barrel Water Level</strong>: """+BARREL_WATER_LEVEL+"""
			<br/><br/><strong>Barrel Overflow Status</strong>: """+OVERFLOW_STATUS+"""
			<br/><br/><strong>Top Tank Overflow</strong>: """+TOP_TANK_OVERFLOW+"""
			</br>

			</p>




			<!-- Modal -->
			<div class="modal fade" id="settingsModal" tabindex="-1" role="dialog" aria-labelledby="exampleModalLabel" aria-hidden="true">
				<div class="modal-dialog" role="document">
				<div class="modal-content">
					<div class="modal-header">
					<h5 class="modal-title" id="exampleModalLabel">Settings</h5>
					<button type="button" class="close" data-dismiss="modal" aria-label="Close">
						<span aria-hidden="true">&times;</span>
					</button>
					</div>
					<div class="modal-body">
					<a href="/reboot.html" class="btn btn-warning" style="color: white;">Soft Reset</a>
					<a href="/shutdownPump.html" class="btn btn-danger" style="color: white;">Shutdown Top Tank Pump</a>
					<a href="/activatePump.html" class="btn btn-success" style="color: white;">Activate Top Tank Pump</a>
					</div>
					<div class="modal-footer">
					</div>
				</div>
				</div>
			</div>



			</body>
			</html>
			"""


			root = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'pi/html')


			if self.path == '/':
				self.send_response(301)
				self.send_header('Location', '/index.html')
				self.end_headers()
			elif self.path == '/index.html':
				content = PAGE.encode('utf-8')
				self.send_response(200)
				self.send_header('Content-Type', 'text/html')
				self.send_header('Content-Length', len(content))
				self.end_headers()
				self.wfile.write(content)
			elif self.path == '/shutdownPump.html':
				message = "Top Tank Pump Shutdown Successfully"
				emergancyTopTankOverflowStop = True
				self.send_response(301)
				self.send_header('Location', '/index.html')
				self.end_headers()
			elif self.path == '/activatePump.html':
				message = "Top Tank Pump Activated Successfully"
				emergancyTopTankOverflowStop = False
				self.send_response(301)
				self.send_header('Location', '/index.html')
				self.end_headers()
			elif self.path == '/reboot.html':
				os.system('sudo shutdown -r now')
				self.send_response(301)
				self.send_header('Location', '/index.html')
				self.end_headers()
			elif self.path == '/purge_cycle.html':
				os.system('sudo shutdown -r now')
				self.send_response(301)
				self.send_header('Location', '/index.html')
				self.end_headers()
			elif self.path.endswith(".css"):
				filename = root + self.path
				self.send_response(200)
				self.send_header('Content-type', 'text/css')
				self.end_headers()
				with open(filename, 'rb') as fh:
					html = fh.read()
					#html = bytes(html, 'utf8')
					self.wfile.write(html)
			elif self.path.endswith(".map"):
				filename = root + self.path
				self.send_response(200)
				self.send_header('Content-type', 'application/json')
				self.end_headers()
				with open(filename, 'rb') as fh:
					html = fh.read()
					#html = bytes(html, 'utf8')
					self.wfile.write(html)
			elif self.path.endswith(".js"):
				filename = root + self.path
				self.send_response(200)
				self.send_header('Content-type', 'application/javascript')
				self.end_headers()
				with open(filename, 'rb') as fh:
					html = fh.read()
					#html = bytes(html, 'utf8')
					self.wfile.write(html)
			elif self.path == '/stream.mjpg':
				self.send_response(200)
				self.send_header('Age', 0)
				self.send_header('Cache-Control', 'no-cache, private')
				self.send_header('Pragma', 'no-cache')
				self.send_header('Content-Type', 'multipart/x-mixed-replace; boundary=FRAME')
				self.end_headers()
				try:
					while True:
						with output.condition:
							output.condition.wait()
							frame = output.frame
						self.wfile.write(b'--FRAME\r\n')
						self.send_header('Content-Type', 'image/jpeg')
						self.send_header('Content-Length', len(frame))
						self.end_headers()
						self.wfile.write(frame)
						self.wfile.write(b'\r\n')
				except Exception as e:
					logging.warning(
						'Removed streaming client %s: %s',
						self.client_address, str(e))
			else:
				self.send_error(404)
				self.end_headers()
		def do_POST(self):
			# Begin the response
			self.send_response(200)
			self.end_headers()
			print(self.client_address)
			print(self.path)
			return
	class StreamingServer(socketserver.ThreadingMixIn, server.HTTPServer):
		allow_reuse_address = True
		daemon_threads = True

	with picamera.PiCamera(resolution='1280x960', framerate=30) as camera:
		output = StreamingOutput()
		#Uncomment the next line to change your Pi's Camera rotation (in degrees)
		camera.rotation = 270
		camera.start_recording(output, format='mjpeg')
		try:
			address = ('', 80)
			server = StreamingServer(address, StreamingHandler)
			server.serve_forever()
		finally:
			camera.stop_recording()

print("Starting Webserver....")
x = threading.Thread(target=webServ, args=())
x.start()

print("Starting AOG....")
tty1 = "/dev/ttyUSB1"
tty2 = "/dev/ttyUSB0"
batcmd="sudo ls /dev/ttyUSB*"
result = subprocess.check_output(batcmd, shell=True)
tty1 = str(result).split("\\n")[0].replace("b'", "")
tty2 = str(result).split("\\n")[1].replace("b'", "")
ser2 = serial.Serial()
ser2.port = tty1
ser2.baudrate = 9600
ser2.open()
ser2.flushInput()
mainLoopErrors = 0


# This pumps water into the main tank
print("Starting Pump Cycle....")
y = threading.Thread(target=cyclePump, args=())
y.start()

# TODO
# Add Amonia, Formaldayde, CO, Light Status, NO2, NO, NH3, O2, etc
while True:
	try:
		print("tty1: " + tty1)
		print("tty2: " + tty2)


		try:
			sensor = sds011.SDS011(tty2, use_query_mode=True )
			sensor.sleep( sleep=False )
			pm25, pm10 = sensor.query()
			print("PM2.5:",pm25,"μg/m^3\nPM10:",pm10,"μg/m^3")
			# Read and record the PM data
			PM_25 = str(pm25) + " &mu;g/m^3"
			PM_10 = str(pm10) + " &mu;g/m^3"
		except Exception as e:
			print(e)

		for i in range(9):
			b = ser2.readline()         # read a byte string
			string_n = b.decode()  # decode byte string into Unicode
			string = string_n.rstrip() # remove \n and \r
			print(string)
			if "S1TVOC" in string:
				TVOC = string.split(": ")[1]
			if "S1CO2" in string:
				S1CO2 = float(string.split(": ")[1].replace("ppm", ""))
			if "S2CO2" in string:
				S2CO2 = float(string.split(": ")[1].replace("ppm", ""))
			if "AVGCO2" in string:
				AVGCO2 = float(string.split(": ")[1].replace("ppm", ""))
			if "HUMIDITY" in string:
				HUMIDITY = float(string.split(": ")[1].replace("%", ""))
			if "TEMPERATURE" in string:
				TEMPERATURE = float(string.split(": ")[1].replace("C", ""))
			if "BARREL_WATER_LEVEL" in string:
				BARREL_WATER_LEVEL = string.split(": ")[1]
			if "BARREL_WATER_OVERFLOW" in string:
				OVERFLOW_STATUS = string.split(": ")[1]
				if "OVERFLOW" in OVERFLOW_STATUS:
					emergancyTopTankOverflowStop = False
			if "TOP_TANK_OVERFLOW" in string:
				TOP_TANK_OVERFLOW = string.split(": ")[1]
				if "OVERFLOW" in TOP_TANK_OVERFLOW:
					emergancyTopTankOverflowStop = True
			TOTALCO2 = str(AVGCO2) + "ppm"
			time.sleep(0.1)            # wait (sleep) 0.1 seconds
			HUMIDITYL = str(HUMIDITY) + "%"
			TEMPERATUREL = str(TEMPERATURE) + "C"

		# Log Data to File
		logData(str(S1CO2)+"ppm", str(S2CO2)+"ppm", TOTALCO2, TVOC, PM_25, PM_10, str(HUMIDITY) + "%", str(TEMPERATURE) + "C")

		# Broadcast servers existance for AOG clients
		server.sendto(broadcastMessage, ('<broadcast>', 37020))
		time.sleep(3)
		mainLoopErrors = 0




	except Exception as e:
		print("Error Code 00000001")
		print(e)
		try:
			try:
				ser2.close()
			except:
				print("Error Code 00000002")
			if mainLoopErrors > 1:
				mainLoopErrors = 0
				batcmd="sudo ls /dev/ttyUSB*"
				result = subprocess.check_output(batcmd, shell=True)
				tty1 = str(result).split("\\n")[1].replace("b'", "")
				tty2 = str(result).split("\\n")[0].replace("b'", "")
				ser2 = serial.Serial()
				ser2.port = tty1
				ser2.baudrate = 9600
				ser2.open()
				ser2.flushInput()
			else:
				mainLoopErrors += 1
				batcmd="sudo ls /dev/ttyUSB*"
				result = subprocess.check_output(batcmd, shell=True)
				tty1 = str(result).split("\\n")[0].replace("b'", "")
				tty2 = str(result).split("\\n")[1].replace("b'", "")
				ser2 = serial.Serial()
				ser2.port = tty1
				ser2.baudrate = 9600
				ser2.open()
				ser2.flushInput()
			print("tty1: " + tty1)
			print("tty2: " + tty2)
		except:
			print("Error Code 00000003")
			# Nothing
