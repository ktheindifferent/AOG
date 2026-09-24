#!/usr/bin/env python3
"""Build both firmware targets with pinned dependencies and package verified HEX files."""
import argparse, hashlib, json, pathlib, shutil, subprocess, zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--cli', default=str(ROOT / '.arduino/bin/arduino-cli'))
parser.add_argument('--config', default=str(ROOT / '.arduino/arduino-cli.yaml'))
parser.add_argument('--install', action='store_true')
args = parser.parse_args()
command = [args.cli, '--config-file', args.config]
if args.install:
    subprocess.run(command + ['core', 'update-index'], check=True)
    subprocess.run(command + ['core', 'install', 'arduino:avr@1.8.6'], check=True)
    for library in ['OneWire@2.3.8', 'Adafruit BusIO@1.17.4', 'Adafruit Unified Sensor@1.1.15', 'Adafruit CCS811 Library@1.1.3', 'DHT sensor library@1.4.6']:
        subprocess.run(command + ['lib', 'install', '--no-deps', library], check=True)
package = ROOT / 'firmware/package'
package.mkdir(exist_ok=True)
manifest = {'version': '2.0.0', 'protocol': 2, 'core': 'arduino:avr@1.8.6', 'sample_interval_seconds': 60, 'buffer_capacity': 24, 'artifacts': []}
for sketch, device in [('sensor-kit','SENSORKIT_MK1'),('dual-ovf','DUAL_OVF_SENSOR')]:
    for board, fqbn in [('nano','arduino:avr:nano:cpu=atmega328'),('nano-old','arduino:avr:nano:cpu=atmega328old'),('uno','arduino:avr:uno')]:
        output = ROOT / 'firmware/build' / f'{sketch}-{board}'
        subprocess.run(command + ['compile','--fqbn',fqbn,'--libraries',str(ROOT/'firmware/libraries'),'--output-dir',str(output),str(ROOT/'firmware'/sketch)], check=True)
        name=f'{sketch}-{board}-2.0.0.hex'
        # Never package the with_bootloader image for an ordinary serial upload.
        source=output/f'{sketch}.ino.hex'
        shutil.copyfile(source,package/name)
        manifest['artifacts'].append({'device':device,'board':board,'fqbn':fqbn,'file':name,'sha256':hashlib.sha256(source.read_bytes()).hexdigest()})
(package/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
with zipfile.ZipFile(package/'sources.zip','w',zipfile.ZIP_DEFLATED) as archive:
    license_info=zipfile.ZipInfo('LICENSE',date_time=(2026,1,1,0,0,0))
    license_info.compress_type=zipfile.ZIP_DEFLATED
    archive.writestr(license_info,(ROOT/'LICENSE').read_bytes())
    for path in sorted((ROOT/'firmware').rglob('*')):
        if path.is_file() and not any(p in {'build','package','__pycache__','sensor-kit-legacy','dual-ovf-legacy'} for p in path.relative_to(ROOT/'firmware').parts):
            info=zipfile.ZipInfo(str(path.relative_to(ROOT)),date_time=(2026,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;archive.writestr(info,path.read_bytes())
print('Packaged six verified firmware profiles and source archive in',package)
