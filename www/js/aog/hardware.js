'use strict';
window.AogHardware = (() => {
  const $ = id => document.getElementById(id);
  const names = {SENSORKIT_MK1: 'Sensor Kit', DUAL_OVF_SENSOR: 'Dual OVF'};
  let api, snapshot, firmware, selected = 'SENSORKIT_MK1', busy = false, dirty = false, initialized = false;
  let saving = false, editRevision = 0, updateError = '';
  let co2Dirty=false,co2Saving=false,co2Revision=0;
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
  const numeric = (value, suffix = '') => {
    if (value === undefined || value === null || value === '') return null;
    const parsed = Number(String(value).replace(suffix, ''));
    return Number.isFinite(parsed) ? parsed : null;
  };
  let traced = 'ccs', sensorRows = [];
  function sensorsFor(device) {
    const connected = fresh(device), f = device?.fields || {}, kit = device?.device_id === 'SENSORKIT_MK1';
    const status = key => !connected ? ['Unavailable','neutral'] : !f[key] ? ['Not reported','neutral'] : f[key] === 'ok' ? ['Reporting','good'] : f[key] === 'not_detected' ? ['Not detected','neutral'] : [f[key].replaceAll('_',' '),'warning'];
    const rows = [], adc = connected ? numeric(f.CO2_ADC) : null;
    const analogValid = Number.isInteger(adc) && adc >= 0 && adc <= 1023;
    const gas=snapshot?.co2_diagnostics,configured=snapshot?.co2_settings?.analog_model==='dfrobot_sen0219_analog';
    const converted=connected && configured?gas?.analog_co2_ppm:null;
    const supply = ['Module supply', 'Rated power', 'power'], ground = ['GND', 'GND', 'ground'];
    if (kit) {
      rows.push({id:'ccs', label:'CCS811 air sensor', value:connected && f.CCS811_STATUS === 'ok' ? f.CO2 : null, unit:'eCO₂ estimate', status:status('CCS811_STATUS'), pin:'A4 / A5', key:snapshot?.co2_settings?.primary_source==='analog'?null:'co2', wires:[['A4','SDA','data'],['A5','SCL','data'],supply,ground], note:`I²C air sensor${connected && f.TVOC ? ` · TVOC ${f.TVOC}` : ''}. Reports equivalent CO₂, not a direct CO₂ measurement.`, help:'Match SDA to A4 and SCL to A5. Use the breakout’s rated supply and logic levels; a bare CCS811 is not a 5 V part. Check the module’s wake/address wiring if it does not report.'});
      rows.push({id:'analog', label:configured?'DFRobot infrared CO₂':'Analog gas input', value:converted!=null?`${Math.round(converted).toLocaleString()} ppm`:analogValid?`${adc} counts`:null, unit:'Raw ADC · 0–1023', status:configured && analogValid?[converted!=null?'Converted reading':'Check signal voltage',converted!=null?'good':'warning']:analogValid?[adc<=1 || adc>=1022?'Check signal voltage':'Unverified analog',adc<=1 || adc>=1022?'warning':'neutral']:['Unavailable','neutral'], pin:'A0', key:snapshot?.co2_settings?.primary_source==='analog'?'co2':null, wires:[['A0','Analog OUT','data'],supply,ground], note:analogValid?`${adc} ADC counts · ≈ ${(adc*(snapshot?.co2_settings?.reference_mv || 5000)/1024000).toFixed(3)} V. ${configured?'SEN0219 v1 analog conversion.':'Unidentified input excluded from CO₂.'}`:'No fresh analog reading. Raw ADC counts do not represent ppm.', help:'Connect the module’s analog output to A0 and share GND. Identify its model and output type first; PWM cannot be read with a voltage-to-ppm formula. Compare A0-to-GND voltage with a meter, then check the rated supply.'});
      rows.push({id:'dht', label:'Air temperature & humidity', value:connected ? [f.TEMP,f.HUM].filter(Boolean).join(' · ') : null, status:status('DHT_STATUS'), pin:'D7', key:'temp', wires:[['D7','DATA','data'],supply,ground], note:'DHT11 supplies climate readings and compensation for the CCS811.', help:'Data connects to D7. Check the module’s rated supply, common GND, data pull-up and sensor model. The installed firmware expects DHT11; another DHT model needs matching firmware.'});
    } else {
      ['T1_OVF','T2_OVF'].forEach((key,i) => {
        const raw=field(device,key), normal=raw==='NONE', triggered=raw==='OVERFLOW';
        rows.push({id:`overflow${i+1}`,label:`Overflow switch ${i+1}`,value:normal?'Circuit closed':triggered?'Circuit open':null,status:[normal?'Clear':triggered?'Stop / check':'Unavailable',normal?'good':triggered?'warning':'neutral'],pin:`D${i?4:2}`,wires:[[`D${i?4:2}`,'Switch signal','data'],['GND','Switch return','ground']],note:'An open circuit can indicate overflow or a disconnected wire.',help:'Connect the switch between the data pin and GND. The input has an internal pull-up. Inspect the float position and cable continuity before interpreting an open circuit as water level.'});
      });
      rows.push({id:'ph',label:'pH probe interface',value:connected && f.PH ? `${f.PH} raw pH` : null,status:[connected && f.PH ? 'Unverified analog' : 'Unavailable','neutral'],pin:'A0',wires:[['A0','Interface OUT','data'],supply,ground],note:connected ? [f.PH_ADC && `ADC ${f.PH_ADC}`,f.PH_MV && `${f.PH_MV} mV`,'Calibrated pH is shown in Overview.'].filter(Boolean).join(' · ') : 'No fresh pH reading.',help:'Connect the interface signal to A0, then attach the probe to its interface. Voltage alone cannot confirm the probe is plugged in. Use known buffers to check calibration in Setup.'});
    }
    [6,8].forEach(pin => {
      const key=`WATER_D${pin}`,rom=f[`${key}_ROM`],detected=rom && rom.startsWith('28');
      rows.push({id:`water${pin}`,label:`Water probe · D${pin}`,value:field(device,key),status:status(`${key}_STATUS`),pin:`D${pin}`,key:`water_temp_d${pin}`,wires:[[`D${pin}`,'DQ / data','data'],['5 V','VDD','power'],ground],note:`${detected?'DS18B20 family detected':'Optional · probe model unconfirmed'}${rom && !/^0+$/.test(rom)?` · ROM ${rom}`:''}.`,help:`For an externally powered DS18B20, use D${pin} for data with its own 4.7 kΩ pull-up to 5 V. Check power, pin labels and ground. Wire colors alone do not identify their functions.`});
    });
    return rows;
  }
  function readingRow(sensor) {
    const item=document.createElement('article'); item.className='sensor-connection'; item.dataset.sensor=sensor.id; item.dataset.status=sensor.status[1];
    const heading=document.createElement('div'); heading.className='sensor-connection-heading';
    const name=document.createElement('h3'); name.textContent=sensor.label;
    const badge=document.createElement('span'); badge.className=`badge ${sensor.status[1]}`; badge.textContent=sensor.status[0];
    heading.append(name,badge);
    const value=document.createElement('strong'); value.className='sensor-raw-value'; value.textContent=sensor.value || '—';
    const note=document.createElement('p'); note.className='helper'; note.textContent=sensor.note;
    const actions=document.createElement('div'); actions.className='sensor-actions';
    const trace=document.createElement('button'); trace.type='button'; trace.className='trace-button'; trace.dataset.trace=sensor.id;
    trace.textContent=`${sensor.pin} · Trace wiring ↗`; trace.setAttribute('aria-label',`Trace wiring for ${sensor.label}`); trace.setAttribute('aria-controls','wiring-inspector'); trace.setAttribute('aria-pressed',String(sensor.id===traced));
    trace.addEventListener('click',()=>chooseTrace(sensor.id,true)); actions.append(trace);
    if(sensor.key) {const link=document.createElement('a');link.className='text-link';link.href=`#history/${sensor.key}`;link.textContent='History →';link.setAttribute('aria-label',`${sensor.label} history`);actions.append(link);}
    item.append(heading,value,note,actions); return item;
  }
  function chooseTrace(id, focus=false) {
    traced=id; renderWiring();
    if(focus) $('wiring-inspector').focus();
  }
  function renderWiring() {
    const sensor=sensorRows.find(s=>s.id===traced) || sensorRows[0]; if(!sensor)return; traced=sensor.id;
    set('trace-title',sensor.label);tag('trace-status',...sensor.status);set('trace-help',sensor.help);
    $('wire-paths').replaceChildren(...sensor.wires.map(([pin,end,kind])=>{
      const row=document.createElement('div');row.className=`wire-path wire-${kind}`;
      for(const [value,cls] of [[pin,'wire-pin'],['','wire-line'],[end,'wire-end']]) {const e=document.createElement('span');e.className=cls;e.textContent=value;row.append(e);}return row;
    }));
    document.querySelectorAll('[data-trace],[data-pin-sensor]').forEach(b=>b.setAttribute('aria-pressed',String((b.dataset.trace || b.dataset.pinSensor)===traced)));
    document.querySelectorAll('[data-sensor]').forEach(e=>e.classList.toggle('is-traced',e.dataset.sensor===traced));
  }
  function renderGas() {
    const device=snapshot.devices.find(d=>d.device_id==='SENSORKIT_MK1'), d=snapshot.co2_diagnostics;
    const live=fresh(device) && d?.fresh;
    const digital=live?d.digital_eco2_ppm:null, adc=live?d.analog_adc:null, volts=live?d.analog_millivolts:null;
    set('gas-digital-value',digital == null?'—':digital.toLocaleString(undefined,{maximumFractionDigits:0}));
    tag('gas-digital-status',digital == null?'Unavailable':d.startup_warmup?'Warming up':'Digital report',digital == null?'neutral':d.startup_warmup?'warning':'good');
    set('gas-digital-detail',digital == null ? d?.source==='legacy_mixed'?'Legacy firmware reports a mixed value; separate digital readings require v2.':'No fresh digital reading is available.' : `${digital===400?'400 ppm is the CCS811’s reporting floor. ':''}${d.compensation==='unavailable'?'DHT error: fresh temperature/humidity compensation is unavailable.':'Equivalent CO₂ estimated from the CCS811’s gas response.'}`);
    const configured=snapshot.co2_settings?.analog_model==='dfrobot_sen0219_analog',analog=live?d.analog_co2_ppm:null;
    set('gas-analog-name',configured?'DFRobot · infrared CO₂':'Analog gas signal');
    set('gas-analog-primary',configured?(analog==null?'—':Math.round(analog).toLocaleString()):(adc==null?'—':adc.toLocaleString()));
    set('gas-analog-unit',configured?'ppm CO₂':'ADC counts / 1023');
    set('gas-analog-value',adc==null?'—':adc.toLocaleString());
    const bad=live && ['near_ground','near_supply','below_range','above_range'].includes(d?.analog_status);
    const primary=d?.source==='dfrobot_sen0219_analog';
    $('gas-analog-primary').closest('.gas-comparison').classList.toggle('infrared-primary',primary);
    tag('gas-analog-status',adc==null?'Unavailable':bad?'Check signal voltage':configured?(primary?'Used for CO₂':'Converted reading'):'Model unconfirmed',bad?'warning':configured?'good':'neutral');
    $('gas-analog-level').style.width=`${adc==null?0:Math.max(0,Math.min(100,adc/1023*100))}%`;
    set('gas-analog-voltage',volts==null?'Voltage estimate unavailable.':`≈ ${(volts/1000).toFixed(3)} V · ${(d.assumed_reference_mv/1000).toFixed(3)} V reference`);
    set('gas-analog-detail',bad?'Voltage is outside the expected output range. Check supply, ground, warm-up and signal wiring. No ppm value is substituted.':configured?'0.4–2.0 V → 0–5,000 ppm · DFRobot SEN0219 v1 analog profile. Supply voltage and sensor calibration affect accuracy.':'ADC counts measure voltage. Identify the module and choose its profile before calculating ppm.');
    set('gas-source-rule',primary?'Dashboard & new history: DFRobot infrared CO₂. CCS811 eCO₂ stays separate. The two readings are never averaged.':d?.source==='ccs811'?'Dashboard & new history: CCS811 eCO₂. The analog input stays separate; the two readings are never averaged.':d?.source==='legacy_mixed'?'Legacy firmware supplies a mixed CO₂ estimate. Upgrade to v2 to separate the analog and digital signals.':'The CO₂ source could not be confirmed. Refresh the connection report.');
    if(!co2Dirty && snapshot.co2_settings) {
      $('co2-model').value=snapshot.co2_settings.analog_model;
      $('co2-primary-source').value=snapshot.co2_settings.primary_source;
      $('co2-reference').value=(snapshot.co2_settings.reference_mv/1000).toFixed(3);
    }
    $('co2-primary-source').querySelector('[value=analog]').disabled=$('co2-model').value==='unidentified';
    $('co2-save').disabled=co2Saving || !co2Dirty;set('co2-save',co2Saving?'Saving…':co2Dirty?'Save CO₂ settings':'CO₂ settings saved');
  }
  function render() {
    if (!snapshot) return;
    const device=selectedDevice(),connected=fresh(device),f=device?.fields || {},kit=selected==='SENSORKIT_MK1';
    const good=snapshot.devices.filter(fresh).length;
    set('hardware-summary',`${good} of 2 Arduino links reporting · inspect individual sensors below`);
    set('hardware-updated',`Checked ${new Date(snapshot.server_time*1000).toLocaleTimeString()}`);
    snapshot.devices.forEach(d=>{
      const short=d.device_id==='SENSORKIT_MK1'?'kit':'ovf', rows=sensorsFor(d), warnings=rows.filter(s=>s.status[1]==='warning').length;
      $('map-'+short).dataset.status=nodeStatus(d);
      set('map-status-'+short,fresh(d)?'● USB reporting':d.observed_at?'○ Offline':'○ Not identified');
      set('map-port-'+short,d.port || 'Discovering USB…');
      set('map-health-'+short,!fresh(d)?'Sensor readings unavailable':`${warnings?`${warnings} sensor${warnings>1?'s':''} need attention`:'No reported sensor errors'} · v${d.fields.FIRMWARE_VERSION || '?'}`);
      $('map-health-'+short).dataset.warning=String(warnings>0 && fresh(d));
    });
    set('inspector-title',`${names[selected]} connections`);set('board-title',names[selected]);
    tag('board-status',connected?'USB reporting':device?.observed_at?'Offline':'Not identified',connected?'good':'warning');
    set('board-port',device?.port || 'Waiting for serial discovery');set('board-seen',stamp(device?.observed_at));
    set('board-version',f.FIRMWARE_VERSION?`v${f.FIRMWARE_VERSION} · protocol ${f.PROTOCOL_VERSION || 'legacy'}`:'Unknown');
    set('board-buffer',f.PROTOCOL_VERSION==='2'?`${f.BUFFER_PENDING || 0} / ${f.BUFFER_CAPACITY || '—'} readings`:'Requires firmware v2');
    set('board-dropped',f.PROTOCOL_VERSION==='2'?`${f.BUFFER_DROPPED || 0} since board reset`:'Not reported by this firmware');
    set('board-clock',f.PROTOCOL_VERSION==='2'?`${f.UPTIME_S || 0}s since board boot`:'No device clock report');
    const dropped=numeric(f.BUFFER_DROPPED)||0,pending=numeric(f.BUFFER_PENDING)||0,capacity=numeric(f.BUFFER_CAPACITY)||24;
    set('board-log-health',!connected?'No current buffer report. Last received details are shown above.':f.PROTOCOL_VERSION!=='2'?'Install firmware v2 for buffered logging.':dropped?`${dropped} readings were dropped since reset. ${pending} waiting for the Pi.`:pending?`${pending} readings waiting to be saved by the Pi.`:'All buffered readings saved. No dropped records.');
    $('board-buffer-level').style.width=`${connected?Math.min(100,pending/capacity*100):0}%`;
    set('board-errors',device?.error || '');$('board-errors').hidden=!device?.error;
    sensorRows=sensorsFor(device);
    if(!sensorRows.some(s=>s.id===traced))traced=sensorRows[0].id;
    const focused=document.activeElement,focusedTrace=focused?.dataset.trace,focusedPin=focused?.dataset.pinSensor;
    const focusedLink=$('sensor-connections').contains(focused)?focused.getAttribute('href'):null;
    $('sensor-connections').replaceChildren(...sensorRows.map(readingRow));
    $('pin-selector').replaceChildren(...sensorRows.map(sensor=>{
      const b=document.createElement('button');b.type='button';b.className='pin-chip';b.dataset.pinSensor=sensor.id;b.textContent=sensor.pin;b.setAttribute('aria-label',`Show ${sensor.label} wiring`);b.addEventListener('click',()=>chooseTrace(sensor.id));return b;
    }));
    renderWiring();
    if(focusedTrace)document.querySelector(`[data-trace="${focusedTrace}"]`)?.focus({preventScroll:true});
    if(focusedPin)document.querySelector(`[data-pin-sensor="${focusedPin}"]`)?.focus({preventScroll:true});
    if(focusedLink)[...$('sensor-connections').querySelectorAll('a')].find(a=>a.getAttribute('href')===focusedLink)?.focus({preventScroll:true});
    set('raw-device-frame',Object.entries(f).map(([k,v])=>`${k}: ${v}`).join('\n') || 'A raw report will appear when this Arduino is identified.');
    set('wiring-heading',`${names[selected]} · connection guide`);
    set('pin-a0-label',kit?'Gas module signal':'pH interface signal');set('pin-d2-label',kit?'Unused':'Overflow switch 1');set('pin-d4-label',kit?'Unused':'Overflow switch 2');set('pin-d7-label',kit?'DHT11 data':'Unused');set('pin-a4-label',kit?'CCS811 SDA':'Unused');set('pin-a5-label',kit?'CCS811 SCL':'Unused');
    if(!dirty)snapshot.water_probes.forEach((probe,i)=>{$(`water-label-${i}`).value=probe.label;$(`water-source-${i}`).value=probe.source || '';});
    snapshot.water_readings.forEach((value,i)=>set(`water-live-${i}`,value?`${value[0].toFixed(2)} °C`:'No unambiguous reading'));
    set('water-save',saving?'Saving…':dirty?'Save probe names & sources':'Probe settings saved');$('water-save').disabled=saving || !dirty;
    renderGas();renderFirmware();
  }
  function renderFirmware() {
    if (!firmware) {
      $('firmware-install').disabled=true;
      $('firmware-hex').hidden=true;
      $('firmware-source').hidden=true;
      set('firmware-tool','Firmware details could not be refreshed. Try refreshing connections.');
      return;
    }
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
      const [connectionResult, firmwareResult] = await Promise.allSettled([api('/api/connections'), api('/api/firmware')]);
      if(connectionResult.status==='rejected')throw connectionResult.reason;
      const data=connectionResult.value,fw=firmwareResult.status==='fulfilled'?firmwareResult.value:null;
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
    window.addEventListener('beforeunload',event=>{if(dirty || co2Dirty){event.preventDefault();event.returnValue='';}});
    $('co2-settings-form').addEventListener('input',()=>{
      co2Dirty=true;co2Revision++;
      const unknown=$('co2-model').value==='unidentified';
      $('co2-primary-source').querySelector('[value=analog]').disabled=unknown;
      if(unknown && $('co2-primary-source').value==='analog')$('co2-primary-source').value='ccs811';
      $('co2-save').disabled=co2Saving;set('co2-save',co2Saving?'Saving…':'Save CO₂ settings');
    });
    $('co2-settings-form').addEventListener('submit',async event=>{
      event.preventDefault();if(co2Saving)return;co2Saving=true;const revision=co2Revision;
      $('co2-save').disabled=true;set('co2-save','Saving…');
      try {
        await api('/api/co2-settings',{body:{analog_model:$('co2-model').value,primary_source:$('co2-primary-source').value,reference_mv:Math.round(Number($('co2-reference').value)*1000)}});
        if(co2Revision===revision)co2Dirty=false;
        set('co2-settings-result',co2Dirty?'Saved the submitted settings. Newer edits still need saving.':'CO₂ settings saved. New readings use the selected source; previous samples are preserved.');
        await refresh();
      }catch(error){set('co2-settings-result',error.message);}
      finally{co2Saving=false;renderGas();}
    });
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
