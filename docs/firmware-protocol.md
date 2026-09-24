# Arduino protocol 2

Serial: 9600 baud, 8-N-1, newline-terminated ASCII, bounded `BEGIN` / `END` frames. Legacy frames remain readable. Device identities are `SENSORKIT_MK1` and `DUAL_OVF_SENSOR`.

Live frames arrive approximately every two seconds. They include `PROTOCOL_VERSION: 2`, `FIRMWARE_VERSION`, `FRAME: LIVE`, `BOOT_ID`, `UPTIME_S`, `BUFFER_PENDING`, `BUFFER_CAPACITY`, `BUFFER_DROPPED`, `SAMPLE_INTERVAL_S`, and `LOGGING`. Sensor fields appear only when valid. Diagnostics include ADC/pH millivolts, DHT/CCS811 states, water ROMs and detection/CRC/power errors.

## Delivery and time

- At first contact, the Pi sends `AOG_SYNC <16-hex-boot-id> <UTC-epoch-seconds>`. The first nonzero boot ID remains fixed until reset; later time corrections must match. Sync repeats each minute.
- Uptime uses unsigned millisecond deltas including `millis()` wrap. Samples retain sequence, sample uptime, and UTC epoch, or zero before synchronization.
- `AOG_DRAIN` enables replay. One oldest `FRAME: LOG` record follows live reports and repeats until acknowledged. It adds `SEQ`, `SAMPLE_UPTIME_S`, and `TIMESTAMP`; `UPTIME_S` is transmission uptime.
- Pre-sync timestamps are estimated as receipt time minus elapsed board uptime and marked `estimated_from_uptime`; clocked samples are marked `synchronized`. Neither uses a battery-backed RTC.
- The Pi appends the complete identity/raw record to a daily JSONL journal, syncs file and directory entries, and **only then** sends `AOG_ACK <boot-id> <sequence>`.
- Only the matching oldest sequence for the current boot is removed. Duplicate/out-of-order ACKs cannot drain other records. The Pi deduplicates `(device, boot ID, sequence)` after retries or restarts.

Storage failure leaves the record pending and displays an error. The ring holds 24 one-minute records; when full it replaces the oldest and increments the drop count. USB reset, flashing, and power loss erase RAM. Normal Linux closes disable HUPCL, but a Pi reboot may still reset a board depending on USB hardware.

## Historical data and live control

Only live frames update overflow and pH files. Replayed “clear” overflow never authorizes pumps or extends live deadlines. Environmental/water replay fills missing minute buckets; valid Pi observations take precedence. Raw journals retain original frames and exact timestamps. pH replay enters chart history under current Pi calibration; unknown tank association uses the neutral 25 °C reference, not current air temperature or an arbitrary water probe.

Journals: `/opt/aog/device_logs/v2/`, retained seven days. Pi history: `/opt/aog/sensor_history/v2/`; original six-column `sensor_history/v1/` files remain readable. These schema names are unrelated to the removed Python application.

`GET /api/device-logs` exports raw records and time quality. `GET /api/history` and reports include recovered temperatures. D6/D8 chart keys stay fixed when display names change.

## Managed updates

`AOG_HOLD` stops new samples while live reports and draining continue. The updater waits for `LOGGING: held` and zero pending records, claims the known port, uploads the verified package, and waits for its expected version. Normal connections send `AOG_RESUME`; failed updates release the hold on reconnection.

The API requires admin session and CSRF. Board profiles and serial paths are allowlisted; no arbitrary URL, command, port, or binary is accepted. Arduino CLI and avrdude have a timeout. Output starts and schedule resumption are blocked during updates. Schedules remain paused on success or failure.
