'use strict';
window.AogHardware = (() => {
  const $ = id => document.getElementById(id);
  const names = {SENSORKIT_MK1: 'Sensor Kit', DUAL_OVF_SENSOR: 'Dual OVF'};
  let api, snapshot, firmware, selected = 'SENSORKIT_MK1', busy = false, dirty = false, initialized = false;
  let saving = false, editRevision = 0, updateError = '';
  const set = (id, value) => {const e = $(id); if (e && e.textContent !== String(value)) e.textContent = value;};
  const fresh = device => snapshot && device && snapshot.server_time - device.observed_at >= 0 && snapshot.server_time - device.observed_at <= 5 && !device.error?.startsWith('Live report failed:');
  const stamp = seconds => seconds ? new Date(seconds * 1000).toLocaleString() : 'No report received';
  const field = (device, key) => fresh(device) ? device.fields[key] : null;
  const selectedDevice = () => snapshot?.devices.find(d => d.device_id === selected);
  const nodeStatus = device => !device?.observed_at ? 'unknown' : fresh(device) ? 'good' : 'bad';
  function tag(id, value, kind) {set(id, value); $(id).className = `badge ${kind}`;}
  function select(id) {
    updateError = '';
    selected = id;
    document.querySelectorAll('[data-board]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.board === id)));
    render();
  }
  function readingRow(label, value, status, detail, key) {
    const item = document.createElement('article'); item.className = 'sensor-connection';
    const heading = document.createElement('div'); heading.className = 'sensor-connection-heading';
    const name = document.createElement('h3'); name.textContent = label;
    const badge = document.createElement('span'); badge.className = `badge ${status[1]}`; badge.textContent = status[0];
    heading.append(name, badge);
    const reading = document.createElement('strong'); reading.className = 'sensor-raw-value'; reading.textContent = value || '—';
    const note = document.createElement('p'); note.className = 'helper'; note.textContent = detail;
    item.append(heading, reading, note);
    if (key) {const link = document.createElement('a'); link.className = 'text-link'; link.href = `#history/${key}`; link.textContent = 'View history →'; item.append(link);}
    return item;
  }
  function render() {
    if (!snapshot) return;
    const device = selectedDevice(), connected = fresh(device), f = device?.fields || {}, kit = selected === 'SENSORKIT_MK1';
    const good = snapshot.devices.filter(fresh).length;
    set('hardware-summary', `${good} of 2 Arduino links reporting`);
    set('hardware-updated', `Last checked ${stamp(snapshot.server_time)}`);
    snapshot.devices.forEach(d => {
      const short = d.device_id === 'SENSORKIT_MK1' ? 'kit' : 'ovf';
      $('map-'+short).dataset.status = nodeStatus(d); $('map-link-'+short).dataset.status = nodeStatus(d);
      set('map-status-'+short, fresh(d) ? `Reporting · v${d.fields.FIRMWARE_VERSION || 'unknown'}` : d.observed_at ? 'No recent report' : 'Waiting for identification');
    });
    set('board-title', names[selected]);
    tag('board-status', connected ? 'USB reporting' : device?.observed_at ? 'Offline' : 'Not identified', connected ? 'good' : 'warning');
    set('board-port', device?.port || 'Waiting for serial discovery');
    set('board-seen', stamp(device?.observed_at));
    set('board-version', f.FIRMWARE_VERSION ? `v${f.FIRMWARE_VERSION} · protocol ${f.PROTOCOL_VERSION || 'legacy'}` : 'Unknown');
    set('board-buffer', f.PROTOCOL_VERSION === '2' ? `${f.BUFFER_PENDING || 0} / ${f.BUFFER_CAPACITY || '—'} readings` : 'Requires firmware v2');
    set('board-dropped', f.PROTOCOL_VERSION === '2' ? `${f.BUFFER_DROPPED || 0} since board reset` : 'Not reported by this firmware');
    set('board-clock', f.PROTOCOL_VERSION === '2' ? `${f.UPTIME_S || 0}s since board boot` : 'No device clock report');
    set('board-errors', device?.error || ''); $('board-errors').hidden = !device?.error;
    const status = key => !connected ? ['Unavailable','neutral'] : !f[key] ? ['Not reported','neutral'] : f[key] === 'ok' ? ['Reporting','good'] : [f[key].replaceAll('_',' '),'warning'];
    const rows = [];
    if (kit) {
      rows.push(readingRow('Air temperature & humidity', connected ? [f.TEMP,f.HUM].filter(Boolean).join(' · ') : null, status('DHT_STATUS'), 'DHT11 data → D7. Check its power and ground if readings fail.', 'temp'));
      rows.push(readingRow('CCS811 air sensor', connected ? [f.CO2 && `${f.CO2} eCO₂`,f.TVOC && `${f.TVOC} TVOC`].filter(Boolean).join(' · ') : null, status('CCS811_STATUS'), 'SDA → A4 · SCL → A5. eCO₂ is an estimate; allow the module to warm up.', 'co2'));
      rows.push(readingRow('Analog gas input', field(device,'CO2_ADC'), [connected && f.CO2_ADC ? 'Analog · unverified' : 'Not reported', 'neutral'], 'Signal → A0. Raw ADC counts (0–1023); module type and calibration are not identified.'));
    } else {
      ['T1_OVF','T2_OVF'].forEach((key,i) => {
        const raw=field(device,key), normal=raw==='NONE', triggered=raw==='OVERFLOW';
        rows.push(readingRow(`Overflow switch ${i+1}`, normal ? 'Circuit closed' : triggered ? 'Circuit open' : null, [normal ? 'Clear' : triggered ? 'Stop / check' : 'Unavailable', normal?'good':triggered?'warning':'neutral'], `Signal → D${i?4:2} · return → GND. An open circuit can mean overflow or a disconnected wire.`));
      });
      rows.push(readingRow('pH probe interface', connected ? [f.PH && `${f.PH} raw pH`,f.PH_ADC && `ADC ${f.PH_ADC}`,f.PH_MV && `${f.PH_MV} mV`].filter(Boolean).join(' · ') : null, [connected && f.PH ? 'Analog · unverified' : 'Unavailable', 'neutral'], 'Interface signal → A0. An analog reading cannot confirm that the probe is plugged in. Check calibration in Setup.'));
    }
    [6,8].forEach(pin => {
      const key = `WATER_D${pin}`, rom=f[`${key}_ROM`], detected=rom && rom.startsWith('28');
      rows.push(readingRow(`Water probe · D${pin}`, field(device,key), status(`${key}_STATUS`), `${detected?'DS18B20 family detected':'Probe model unconfirmed'}${rom && !/^0+$/.test(rom)?` · ROM ${rom}`:''}. Data → D${pin}; powered 1-Wire requires a pull-up.`, `water_temp_d${pin}`));
    });
    const focusedLink = $('sensor-connections').contains(document.activeElement) ? document.activeElement.getAttribute('href') : null;
    $('sensor-connections').replaceChildren(...rows);
    if (focusedLink) [...$('sensor-connections').querySelectorAll('a')].find(a => a.getAttribute('href') === focusedLink)?.focus({preventScroll:true});
    set('raw-device-frame', Object.entries(f).map(([k,v]) => `${k}: ${v}`).join('\n') || 'A raw report will appear when this Arduino is identified.');
    set('wiring-heading', `${names[selected]} · connection guide`);
    set('pin-a0-label', kit ? 'Gas module signal' : 'pH interface signal');
    set('pin-d2-label', kit ? 'Unused' : 'Overflow switch 1');
    set('pin-d4-label', kit ? 'Unused' : 'Overflow switch 2');
    set('pin-d7-label', kit ? 'DHT11 data' : 'Unused');
    set('pin-a4-label', kit ? 'CCS811 SDA' : 'Unused');
    set('pin-a5-label', kit ? 'CCS811 SCL' : 'Unused');
    if (!dirty) snapshot.water_probes.forEach((probe,i) => {$(`water-label-${i}`).value=probe.label; $(`water-source-${i}`).value=probe.source || '';});
    snapshot.water_readings.forEach((value,i) => set(`water-live-${i}`, value ? `${value[0].toFixed(2)} °C` : 'No unambiguous reading'));
    set('water-save', saving ? 'Saving…' : dirty ? 'Save probe names & sources' : 'Probe settings saved');
    $('water-save').disabled = saving || !dirty;
    renderFirmware();
  }
  function renderFirmware() {
    if (!firmware) return;
    set('firmware-version', firmware.package ? `v${firmware.package.version}` : 'Package unavailable');
    set('firmware-tool', firmware.updater_available ? 'Upload tools installed' : 'Upload tools are not installed on this Pi');
    const job = firmware.job;
    set('firmware-job', updateError || (job?.message ? `${names[job.device] ? names[job.device] + ' · ' : ''}${job.message}` : 'Choose the Arduino and its board profile to install the packaged firmware.'));
    $('firmware-job').classList.toggle('error', Boolean(updateError) || job?.stage === 'failed');
    $('firmware-progress').hidden = !firmware.active;
    $('firmware-install').disabled = !firmware.updater_available || !firmware.package || firmware.active || !fresh(selectedDevice()) || !$('firmware-board').value;
    set('firmware-install', firmware.active ? 'Update in progress…' : `Install on ${names[selected]}`);
    set('firmware-device', names[selected]);
    const artifact = firmware.package?.artifacts.find(a => a.device === selected && a.board === $('firmware-board').value);
    $('firmware-hex').hidden = !artifact;
    if (artifact) $('firmware-hex').href = `/api/firmware/download?file=${encodeURIComponent(artifact.file)}`;
    set('firmware-hash', artifact ? `SHA-256 · ${artifact.sha256}` : 'Choose a board profile to inspect its binary checksum.');
    $('firmware-source').hidden = !firmware.package;
  }
  async function refresh() {
    if (!api || busy) return;
    busy=true;
    try {
      const [data, fw] = await Promise.all([api('/api/connections'), api('/api/firmware')]);
      if (!Array.isArray(data.devices) || data.devices.length!==2 || !Array.isArray(data.water_probes) || !Array.isArray(data.water_readings)) throw new Error('Invalid connection report');
      snapshot=data;firmware=fw;render();$('hardware-error').hidden=true;
    } catch(error) {
      set('hardware-error', `Connection details could not be refreshed. ${error.message}`);$('hardware-error').hidden=false;
      // Invalidate old green states immediately when the Pi cannot be reached.
      if (snapshot) {snapshot.server_time = Math.max(Date.now()/1000,...snapshot.devices.map(d=>d.observed_at+9));snapshot.water_readings=[null,null];render();}
    } finally {busy=false;}
  }
  function init(client) {
    api=client;if(initialized) return;initialized=true;
    document.querySelectorAll('[data-board]').forEach(button => button.addEventListener('click', () => select(button.dataset.board)));
    $('hardware-refresh').addEventListener('click',refresh);
    $('firmware-board').addEventListener('change',()=>{updateError='';renderFirmware();});
    window.addEventListener('beforeunload',event=>{if(dirty){event.preventDefault();event.returnValue='';}});
    $('water-probes-form').addEventListener('input', () => {dirty=true;editRevision++;$('water-save').disabled=saving;set('water-save',saving?'Saving…':'Save probe names & sources');});
    $('water-probes-form').addEventListener('submit',async event => {
      event.preventDefault();if(saving)return;saving=true;const submittedRevision=editRevision;$('water-save').disabled=true;set('water-save','Saving…');
      try {
        await api('/api/water-probes',{body:[0,1].map(i=>({label:$(`water-label-${i}`).value.trim(),source:$(`water-source-${i}`).value||null}))});
        if(editRevision===submittedRevision)dirty=false;
        set('water-settings-result',dirty?'Saved the submitted settings. Your newer edits still need saving.':'Probe settings saved. History keys remain D6 and D8.');await refresh();
      } catch(error) {set('water-settings-result',error.message);}
      finally {saving=false;render();}
    });
    $('firmware-install').addEventListener('click',async () => {
      updateError='';$('firmware-install').disabled=true;
      try {await api('/api/firmware/update',{body:{device:selected,board:$('firmware-board').value}});await refresh();}
      catch(error) {updateError=error.message;renderFirmware();}
    });
    setInterval(()=>{if (!document.hidden && (location.hash==='#connections' || firmware?.active)) refresh();},3000);
    select(selected);
    if(location.hash==='#connections') refresh();
  }
  return {init,refresh};
})();
