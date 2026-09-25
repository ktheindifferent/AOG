'use strict';
window.AogHardware = (() => {
  const $ = id => document.getElementById(id);
  const names = {SENSORKIT_MK1: 'Sensor Kit', DUAL_OVF_SENSOR: 'Dual OVF'};
  let api, snapshot, firmware, selected = 'SENSORKIT_MK1', busy = false, dirty = false, initialized = false;
  let saving = false, editRevision = 0, updateError = '';
  let co2Dirty=false,co2Saving=false,co2Revision=0;
  let phSaving=false;
  let receivedAt=0, connectionFailed=false, lastFreshness='', inspectionView='wiring';
  const set = (id, value) => {const e = $(id); if (e && e.textContent !== String(value)) e.textContent = value;};
  // Elapsed browser time also ages a report while an HTTP request is slow or pending.
  const reportAge = device => snapshot && device?.observed_at ? snapshot.server_time-device.observed_at+(performance.now()-receivedAt)/1000 : null;
  const fresh = device => !connectionFailed && reportAge(device)!==null && reportAge(device)>=0 && reportAge(device)<=5 && !device.error?.startsWith('Live report failed:');
  const stamp = seconds => seconds ? new Date(seconds * 1000).toLocaleString() : 'No report received';
  const field = (device, key) => fresh(device) ? device.fields[key] : null;
  const selectedDevice = () => snapshot?.devices.find(d => d.device_id === selected);
  const nodeStatus = device => !device?.observed_at ? 'unknown' : fresh(device) ? 'good' : 'bad';
  function tag(id, value, kind) {set(id, value); $(id).className = `badge ${kind}`;}
  function select(id) {
    updateError = '';
    if (traced) remembered[selected] = traced;
    selected = id;
    traced = remembered[id] || null;
    sensorFilter = 'all';
    mobileInspectorOpen = false;
    document.querySelectorAll('[data-board]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.board === id)));
    render();
  }
  const numeric = (value, suffix = '') => {
    if (value === undefined || value === null || value === '') return null;
    const parsed = Number(String(value).replace(suffix, ''));
    return Number.isFinite(parsed) ? parsed : null;
  };
  const displayNumber = (value, suffix, unit) => {
    const number=numeric(value,suffix);
    return number===null?null:`${number.toLocaleString(undefined,{maximumFractionDigits:2})}${unit}`;
  };
  let traced = null, sensorRows = [], sensorFilter = 'all', mobileInspectorOpen = false;
  const remembered = {};
  const narrowInspector = matchMedia('(max-width: 780px)');
  const sensorFields = {
    ccs: ['CCS811_STATUS','CO2','TVOC'], analog: ['CO2_ADC'],
    dht: ['DHT_STATUS','TEMP','HUM'], overflow1: ['T1_OVF'], overflow2: ['T2_OVF'],
    ph: ['PH_STATUS','PH','PH_ADC','PH_MV'],
    water6: ['WATER_D6_STATUS','WATER_D6','WATER_D6_ROM'],
    water8: ['WATER_D8_STATUS','WATER_D8','WATER_D8_ROM']
  };
  const statusLabels = {
    crc_error:'Data check failed', unsupported_family:'Unsupported probe',
    out_of_range:'Outside sensor range', multiple_probes:'Multiple probes',
    external_power_required:'External power needed', read_error:'Read failed', unavailable:'No sensor response'
  };
  const sensorStatus = (connected,raw) => !connected?['Unavailable','neutral']:!raw?['Not reported','neutral']:raw==='ok'?['Reporting','good']:raw==='not_detected'?['Not detected','neutral']:[statusLabels[raw] || raw.replaceAll('_',' '),'warning'];
  function sensorsFor(device) {
    const connected = fresh(device), f = device?.fields || {}, kit = device?.device_id === 'SENSORKIT_MK1';
    const status = key => sensorStatus(connected,f[key]);
    const rows = [], adc = connected ? numeric(f.CO2_ADC) : null;
    const analogValid = Number.isInteger(adc) && adc >= 0 && adc <= 1023;
    const gas=snapshot?.co2_diagnostics,configured=snapshot?.co2_settings?.analog_model==='dfrobot_sen0219_analog';
    const converted=connected && configured?gas?.analog_co2_ppm:null;
    const supply = ['Module supply', 'Rated power', 'power'], ground = ['GND', 'GND', 'ground'];
    if (kit) {
      rows.push({id:'ccs', label:'CCS811 air sensor', value:connected && f.CCS811_STATUS === 'ok' ? displayNumber(f.CO2,'ppm',' ppm eCO₂') : null, status:status('CCS811_STATUS'), pin:'A4 / A5', key:snapshot?.co2_settings?.primary_source==='analog'?null:'co2', wires:[['A4','SDA','data'],['A5','SCL','data'],supply,ground], note:`I²C air sensor${connected && f.TVOC ? ` · TVOC ${f.TVOC}` : ''}. Reports equivalent CO₂, not a direct CO₂ measurement.`, help:'Match SDA to A4 and SCL to A5. Use the breakout’s rated supply and logic levels; a bare CCS811 is not a 5 V part. Check the module’s wake/address wiring if it does not report.'});
      rows.push({id:'analog', label:configured?'DFRobot infrared CO₂':'Analog gas input', value:converted!=null?`${Math.round(converted).toLocaleString()} ppm`:analogValid?`${adc} counts`:null, status:configured && analogValid?[converted!=null?(snapshot?.co2_settings?.primary_source==='analog'?'Used for CO₂':'Converted reading'):'Check signal voltage',converted!=null?'good':'warning']:analogValid?[adc<=1 || adc>=1022?'Check signal voltage':'Unverified analog',adc<=1 || adc>=1022?'warning':'neutral']:['Unavailable','neutral'], pin:'A0', key:snapshot?.co2_settings?.primary_source==='analog'?'co2':null, wires:[['A0','Analog OUT','data'],supply,ground], note:analogValid?`${adc} ADC counts · ≈ ${(adc*(snapshot?.co2_settings?.reference_mv || 5000)/1024000).toFixed(3)} V. ${configured?'SEN0219 v1 analog conversion.':'Unidentified input excluded from CO₂.'}`:'No fresh analog reading. Raw ADC counts do not represent ppm.', help:'Connect the module’s analog output to A0 and share GND. Identify its model and output type first; PWM cannot be read with a voltage-to-ppm formula. Compare A0-to-GND voltage with a meter, then check the rated supply.'});
      rows.push({id:'dht', label:'Air temperature & humidity', value:connected && f.DHT_STATUS==='ok' ? [displayNumber(f.TEMP,'C',' °C'),displayNumber(f.HUM,'%','%')].filter(Boolean).join(' · ') : null, status:status('DHT_STATUS'), pin:'D7', key:'temp', wires:[['D7','DATA','data'],supply,ground], note:'DHT11 supplies climate readings and compensation for the CCS811.', help:'Data connects to D7. Check the module’s rated supply, common GND, data pull-up and sensor model. The installed firmware expects DHT11; another DHT model needs matching firmware.'});
    } else {
      ['T1_OVF','T2_OVF'].forEach((key,i) => {
        const raw=field(device,key), normal=raw==='NONE', triggered=raw==='OVERFLOW';
        rows.push({id:`overflow${i+1}`,label:`Overflow switch ${i+1}`,value:normal?'Circuit closed':triggered?'Circuit open':null,status:[normal?'Clear':triggered?'Stop / check':'Unavailable',normal?'good':triggered?'warning':'neutral'],pin:`D${i?4:2}`,wires:[[`D${i?4:2}`,'Switch signal','data'],['GND','Switch return','ground']],note:'An open circuit can indicate overflow or a disconnected wire.',help:'Connect the switch between the data pin and GND. The input has an internal pull-up. Inspect the float position and cable continuity before interpreting an open circuit as water level.'});
      });
      const adc=connected?numeric(f.PH_ADC):null,enabled=Boolean(snapshot?.ph_input?.connected);
      rows.push({id:'ph',label:'Atlas Gravity pH input',value:adc!==null?`${adc} ADC counts`:null,status:!connected?['Unavailable','neutral']:f.PH_STATUS==='out_of_range'?['Check analog signal','warning']:enabled?['Wiring confirmed by user','neutral']:['Connection not confirmed','warning'],pin:'A0',wires:[['A0','Gravity A · signal','data'],['5V','Gravity + · power','power'],['GND','Gravity − · ground','ground']],note:connected ? [f.PH_MV && `${f.PH_MV} mV`,enabled?'Analog voltage cannot detect an unplugged probe.':'Excluded from water pH until the physical connection is confirmed.'].filter(Boolean).join(' · ') : 'No fresh analog report.',help:'Atlas Gravity V2.1: A to A0, + to 5V, − to GND on Dual OVF. Keep the probe attached to the Gravity board. Open pH connection setup below, confirm the wiring, then check known buffers and recalibrate.'});
    }
    [6,8].forEach(pin => {
      const key=`WATER_D${pin}`,rom=f[`${key}_ROM`],detected=rom && rom.startsWith('28');
      rows.push({id:`water${pin}`,label:`Water probe · D${pin}`,value:f[`${key}_STATUS`]==='ok'?displayNumber(field(device,key),'C',' °C'):null,status:status(`${key}_STATUS`),pin:`D${pin}`,key:`water_temp_d${pin}`,wires:[[`D${pin}`,'DQ / data','data'],['5 V','VDD','power'],ground],note:`${detected?'DS18B20 family detected':'Optional · probe model unconfirmed'}${rom && !/^0+$/.test(rom)?` · ROM ${rom}`:''}.`,help:`For an externally powered DS18B20, use D${pin} for data with its own 4.7 kΩ pull-up to 5 V. Check power, pin labels and ground. Wire colors alone do not identify their functions.`});
    });
    return rows;
  }
  function diagnosis(sensor,device) {
    const f=device?.fields || {}, raw=sensor.id.startsWith('water')?f[`WATER_D${sensor.id.slice(5)}_STATUS`]:null;
    if(connectionFailed)return ['Live view unavailable','AOG could not refresh connection details. Raw data keeps the last received report.'];
    if(!fresh(device))return ['Waiting for a board report','Live values are hidden until this Arduino reports again. Check its USB connection and power; the last received fields remain in Raw data.'];
    if(!sensor.value && sensor.status[1]!=='warning' && !sensor.id.startsWith('water'))return ['No usable sensor value received','The board is reporting, but this sensor has no usable value. Check Raw data for its status and the connection checklist for its wiring.'];
    if(sensor.id.startsWith('overflow'))return sensor.status[1]==='warning'
      ? ['An open circuit needs a physical check','Overflow and a disconnected wire can both open this circuit. Inspect the float position and cable continuity before interpreting the water level.']
      : ['Switch circuit is closed','The Arduino reads a closed path to ground. This describes the switch circuit, not a measured tank depth.'];
    if(sensor.id.startsWith('water')) {
      const messages={
        not_detected:['No probe detected on this pin','This input is optional. If a probe is fitted, confirm its model, power and data pin before interpreting this status.'],
        crc_error:['The data check failed','The probe’s data did not pass its integrity check. Check the data cable, shared ground and pull-up; this sample is not used as a temperature reading.'],
        unsupported_family:['This probe type is not supported','The returned device family does not match DS18B20 (0x28). Identify the probe model before changing its wiring.'],
        multiple_probes:['More than one probe shares this pin','The firmware expects one probe per input. Give the two probes separate data lines on D6 and D8.'],
        external_power_required:['The probe needs external power','The firmware detected a probe without external power. For a confirmed DS18B20, check VDD, GND and the data pull-up shown in Probe setup.'],
        out_of_range:['Temperature was rejected','The returned value is outside the firmware’s sensor range. Check the probe model, cable and supply before using the reading.'],
        ok:['A valid temperature was received','The detected DS18B20 passed the firmware’s data check. Raw data includes its ROM identifier.']
      };
      return messages[raw] || ['No probe status received','Inspect the raw fields and confirm that the Arduino is running firmware with water-probe support.'];
    }
    if(sensor.id==='analog') {
      if(sensor.status[1]==='warning')return ['Check the analog signal','A0 is at a voltage limit or outside the selected profile’s range. Check the module’s supply, ground and output type; no replacement ppm value is used.'];
      if(snapshot.co2_settings?.analog_model!=='dfrobot_sen0219_analog')return ['The analog module still needs a profile','ADC counts describe voltage. Identify the module and choose its profile before interpreting this signal as CO₂.'];
      return ['The analog signal is in range','The configured SEN0219 v1 profile converts this voltage into CO₂. Raw data shows the counts, voltage and conversion separately.'];
    }
    if(sensor.id==='ph')return snapshot?.ph_input?.connected
      ? ['Analog connection requires a physical check','You confirmed this Gravity input. Voltage cannot independently detect unplugging; disable the pH input before disconnecting it and use known buffers to check measurements.']
      : ['A voltage is not proof of a connected probe','A floating A0 can produce plausible pH numbers. This input is excluded from water measurements until you confirm the three-wire connection in pH connection setup below.'];
    if(sensor.id==='dht')return sensor.status[1]==='warning'
      ? ['The climate reading failed','Check the DHT11 data connection on D7 and its rated supply. A failed reading also prevents fresh temperature/humidity compensation for CCS811.']
      : ['Climate data is reporting','DHT11 supplies both temperature and humidity. These readings also compensate the CCS811 estimate.'];
    return sensor.status[1]==='warning'
      ? ['The CCS811 is not reporting','Check SDA on A4, SCL on A5, and the breakout’s rated power, wake and address connections.']
      : ['The digital estimate is reporting','CCS811 reports equivalent CO₂ from its gas response. It remains separate from the infrared CO₂ reading.'];
  }
  function renderChecks() {
    const checks=[];
    if(!connectionFailed)snapshot.devices.forEach(device=>{
      if(!fresh(device)) {
        checks.push({id:`${device.device_id}:link`,device:device.device_id,label:names[device.device_id],detail:device.observed_at?'No fresh board report':'Arduino not identified'});
        return;
      }
      sensorsFor(device).filter(s=>s.status[1]==='warning').forEach(sensor=>checks.push({id:`${device.device_id}:${sensor.id}`,device:device.device_id,sensor:sensor.id,label:`${names[device.device_id]} · ${sensor.pin}`,detail:`${sensor.label} · ${sensor.status[0]}`}));
      const dropped=numeric(device.fields.BUFFER_DROPPED);
      if(dropped>0)checks.push({id:`${device.device_id}:logs`,device:device.device_id,label:`${names[device.device_id]} · logs`,detail:`${dropped} record${dropped===1?'':'s'} dropped since board reset`});
    });
    const list=$('connection-check-list'),keys=new Set(checks.map(c=>c.id));
    list.querySelectorAll('[data-check]').forEach(button=>{
      if(!keys.has(button.dataset.check)) {
        if(button===document.activeElement)document.querySelector(`[data-board="${button.dataset.check.split(':')[0]}"]`)?.focus({preventScroll:true});
        button.remove();
      }
    });
    checks.forEach(check=>{
      let button=[...list.children].find(b=>b.dataset.check===check.id);
      if(!button) {
        button=document.createElement('button');button.type='button';button.className='connection-check';button.dataset.check=check.id;
        button.innerHTML='<span><strong></strong><span></span></span><span aria-hidden="true">↗</span>';
        button.addEventListener('click',()=>{
          select(check.device);
          if(check.sensor) {setInspectionView('wiring');chooseTrace(check.sensor,true);}
          else openTool('board-details');
        });
        list.append(button);
      }
      const label=button.querySelector('strong'),detail=button.querySelector('span>span');
      if(label.textContent!==check.label)label.textContent=check.label;
      if(detail.textContent!==check.detail)detail.textContent=check.detail;
      button.setAttribute('aria-label',`${check.label}: ${check.detail}. Inspect connection`);
    });
    $('connection-checks').hidden=!checks.length;
    set('connection-checks-title',`${checks.length} hardware check${checks.length===1?'':'s'}`);
  }
  function renderReportAge() {
    const device=selectedDevice(),age=reportAge(device);
    const elapsed=age===null?'No report received':age<0?'Report time is ahead of the controller':age<60?`${Math.floor(age)}s ago`:age<3600?`${Math.floor(age/60)}m ago`:`${Math.floor(age/3600)}h ago`;
    set('trace-freshness',`${fresh(device)?'Live report':connectionFailed?'Last received':device?.observed_at?'Last report':'Waiting'} · ${elapsed}`);
    $('trace-freshness').dataset.live=String(fresh(device));
    $('trace-freshness').title=stamp(device?.observed_at);
  }
  function setInspectionView(view,focus=false) {
    inspectionView=view;
    document.querySelectorAll('[data-inspection-view]').forEach(tab=>{
      const active=tab.dataset.inspectionView===view;
      tab.setAttribute('aria-selected',String(active));tab.tabIndex=active?0:-1;
      $(tab.getAttribute('aria-controls')).hidden=!active;
      if(active && focus)tab.focus({preventScroll:true});
    });
  }
  function readingRow(sensor) {
    const item=document.createElement('article'); item.className='sensor-connection'; item.dataset.sensor=sensor.id;
    const trace=document.createElement('button'); trace.type='button'; trace.className='trace-button'; trace.dataset.trace=sensor.id;
    trace.setAttribute('aria-controls','wiring-inspector');
    // Keep these controls mounted between polls so keyboard focus and pointer clicks remain stable.
    trace.innerHTML='<span class="sensor-pin"></span><span class="sensor-row-body"><span class="sensor-name"></span><span class="sensor-row-reading"></span><span class="sensor-row-status"></span></span><span class="sensor-arrow" aria-hidden="true">↗</span>';
    trace.addEventListener('click',()=>{
      if(narrowInspector.matches && mobileInspectorOpen && traced===sensor.id)backToSensors();
      else chooseTrace(sensor.id,true);
    });
    item.append(trace); return item;
  }
  function renderSensors() {
    const list=$('sensor-connections'), ids=new Set(sensorRows.map(s=>s.id));
    list.querySelectorAll('[data-sensor]').forEach(row=>{if(!ids.has(row.dataset.sensor))row.remove();});
    sensorRows.forEach(sensor=>{
      let row=list.querySelector(`[data-sensor="${sensor.id}"]`);
      if(!row) {row=readingRow(sensor);list.append(row);}
      row.dataset.status=sensor.status[1];
      const update=(selector,value)=>{const el=row.querySelector(selector);if(el.textContent!==value)el.textContent=value;};
      update('.sensor-pin',sensor.pin);update('.sensor-name',sensor.label);
      update('.sensor-row-reading',sensor.value || '—');update('.sensor-row-status',sensor.status[0]);
      row.querySelector('[data-trace]').setAttribute('aria-label',`Trace wiring for ${sensor.label}`);
      // History must follow the selected CO₂ source, including after a settings change.
      let link=row.querySelector('a');
      if(sensor.key) {
        if(!link) {link=document.createElement('a');link.className='sensor-history';link.textContent='History ↗';row.append(link);}
        link.href=`#history/${sensor.key}`;link.setAttribute('aria-label',`${sensor.label} history`);
      } else if(link)link.remove();
    });
    // A board switch shares D6/D8 with the previous board: restore the intended order once.
    const current=[...list.querySelectorAll('[data-sensor]')];
    if(current.some((row,i)=>row.dataset.sensor!==sensorRows[i].id))sensorRows.forEach(sensor=>list.append(list.querySelector(`[data-sensor="${sensor.id}"]`)));
    const chips=$('pin-selector');
    chips.querySelectorAll('[data-pin-sensor]').forEach(b=>{if(!ids.has(b.dataset.pinSensor))b.remove();});
    sensorRows.forEach(sensor=>{
      let b=chips.querySelector(`[data-pin-sensor="${sensor.id}"]`);
      if(!b) {b=document.createElement('button');b.type='button';b.className='pin-chip';b.dataset.pinSensor=sensor.id;b.addEventListener('click',()=>chooseTrace(sensor.id,true));chips.append(b);}
      if(b.textContent!==sensor.pin)b.textContent=sensor.pin;
      b.setAttribute('aria-label',`Show ${sensor.label} wiring`);b.setAttribute('aria-controls','wiring-inspector');
    });
    const currentChips=[...chips.children];
    if(currentChips.some((b,i)=>b.dataset.pinSensor!==sensorRows[i].id))sensorRows.forEach(sensor=>chips.append(chips.querySelector(`[data-pin-sensor="${sensor.id}"]`)));
    renderFilter();
  }
  function renderFilter() {
    const warnings=sensorRows.filter(s=>s.status[1]==='warning');
    if(sensorFilter==='attention' && warnings.length && !warnings.some(s=>s.id===traced))traced=warnings[0].id;
    set('sensor-count',sensorRows.length);set('sensor-all-count',sensorRows.length);set('sensor-attention-count',warnings.length);
    $('sensor-attention-count').dataset.warning=String(warnings.length>0);
    document.querySelectorAll('[data-sensor-filter]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.sensorFilter===sensorFilter)));
    sensorRows.forEach(sensor=>{document.querySelector(`[data-sensor="${sensor.id}"]`).hidden=sensorFilter==='attention' && sensor.status[1]!=='warning';});
    $('sensor-empty').hidden=sensorFilter!=='attention' || warnings.length>0;
    set('sensor-empty',fresh(selectedDevice())?'No reported sensor warnings. Choose All to see optional and unverified connections.':'No fresh sensor checks are available. Choose All to inspect wiring and the last received data.');
    const empty=sensorFilter==='attention' && !warnings.length;
    if(empty && $('wiring-inspector').contains(document.activeElement))document.querySelector('[data-sensor-filter="attention"]').focus({preventScroll:true});
    const hidden=empty || (narrowInspector.matches && !mobileInspectorOpen);
    $('wiring-inspector').hidden=hidden;$('wiring-dock').hidden=hidden;
  }
  function chooseTrace(id, focus=false) {
    traced=id;remembered[selected]=id;
    mobileInspectorOpen=true;
    const sensor=sensorRows.find(s=>s.id===id);
    if(sensorFilter==='attention' && sensor?.status[1]!=='warning')sensorFilter='all';
    renderFilter();
    renderWiring();
    if(focus) {
      set('connection-announcement',`${sensor?.label || 'Sensor'} wiring selected.`);
      $('wiring-inspector').focus({preventScroll:true});
      $('wiring-inspector').scrollIntoView({block:narrowInspector.matches?'start':'nearest',behavior:'instant'});
    }
  }
  function placeInspector() {
    const panel=$('wiring-inspector'), row=document.querySelector(`[data-sensor="${traced}"]`);
    const inline=narrowInspector.matches && mobileInspectorOpen && row && !row.hidden;
    const needsMove=inline?row.nextElementSibling!==panel:panel.parentElement!==$('wiring-dock');
    if(!needsMove)return;
    const active=panel.contains(document.activeElement)?document.activeElement:null;
    if(inline)row.after(panel);else $('wiring-dock').append(panel);
    if(active)active.focus({preventScroll:true});
  }
  function backToSensors() {
    if(sensorFilter==='attention' && !sensorRows.some(s=>s.id===traced && s.status[1]==='warning'))sensorFilter='all';
    if(narrowInspector.matches)mobileInspectorOpen=false;
    // Return focus to the selected row before moving the inspector out of the list.
    renderFilter();
    document.querySelector(`[data-trace="${traced}"]`)?.focus();
    renderWiring();
  }
  function openTool(id) {
    const target=$(id);if(!target)return;
    target.open=true;
    target.querySelector('summary').focus({preventScroll:true});
    target.scrollIntoView({block:'start',behavior:'instant'});
  }
  function renderWiring() {
    const sensor=sensorRows.find(s=>s.id===traced) || sensorRows[0]; if(!sensor)return; traced=sensor.id;
    set('trace-title',sensor.label);tag('trace-status',...sensor.status);set('trace-help',sensor.help);
    set('trace-board',`${names[selected]} / ${sensor.pin}`);set('trace-value',sensor.value || '—');set('trace-note',sensor.note);set('trace-pin',sensor.pin);
    set('trace-reading-label',fresh(selectedDevice())?'LATEST READING':'NO FRESH READING');
    set('trace-help-label',sensor.status[1]==='warning'?'Connection checks to try':'Connection checklist');
    $('wiring-inspector').dataset.status=sensor.status[1];
    const [title,detail]=diagnosis(sensor,selectedDevice());
    set('diagnosis-title',title);set('diagnosis-detail',detail);
    $('sensor-diagnosis').dataset.status=sensor.status[1];
    set('diagnosis-symbol',sensor.status[1]==='warning'?'!':sensor.status[1]==='good'?'✓':'i');
    $('trace-history').hidden=!sensor.key;
    $('trace-ph-setup').hidden=sensor.id!=='ph';
    if(sensor.key) {$('trace-history').href=`#history/${sensor.key}`;$('trace-history').setAttribute('aria-label',`${sensor.label} history`);}
    const tool=['ccs','analog'].includes(sensor.id)?'co2-tools':sensor.id.startsWith('water')?'water-tools':null;
    $('trace-setup').hidden=!tool;
    $('trace-setup').dataset.hardwareJump=tool || '';
    set('trace-setup',tool==='co2-tools'?'CO₂ source & sensor profile →':'Probe setup & full wiring diagram →');
    const fields=selectedDevice()?.fields || {}, keys=sensorFields[sensor.id] || [];
    const entries=keys.filter(key=>fields[key]!==undefined);
    set('trace-raw-count',entries.length);
    const table=$('raw-sensor-frame');
    if(table.dataset.fields!==entries.join(',')) {
      table.dataset.fields=entries.join(',');
      table.replaceChildren(...entries.map(key=>{
        const row=document.createElement('div');row.dataset.field=key;
        const term=document.createElement('dt'),value=document.createElement('dd');term.textContent=key;row.append(term,value);return row;
      }));
    }
    entries.forEach(key=>{const value=table.querySelector(`[data-field="${key}"] dd`);if(value.textContent!==fields[key])value.textContent=fields[key];});
    $('trace-no-fields').hidden=entries.length>0;$('trace-download').disabled=!entries.length;
    set('trace-raw-stamp',`${fresh(selectedDevice())?'Received':'Stored report'} · ${stamp(selectedDevice()?.observed_at)}.${fresh(selectedDevice())?'':' These fields are not live.'}`);
    $('trace-conversion').hidden=sensor.id!=='analog';
    if(sensor.id==='analog') {
      const gas=snapshot.co2_diagnostics,live=fresh(selectedDevice()) && gas?.fresh;
      const profile=snapshot.co2_settings?.analog_model==='dfrobot_sen0219_analog';
      set('trace-adc',live && gas.analog_adc!=null?gas.analog_adc:'—');
      set('trace-volts',live && gas.analog_millivolts!=null?(gas.analog_millivolts/1000).toFixed(3):'—');
      set('trace-ppm',live && gas.analog_co2_ppm!=null?Math.round(gas.analog_co2_ppm).toLocaleString():'—');
      set('trace-ppm-label',profile?'ppm CO₂':'Profile needed');
      set('trace-conversion-note',!live?'No fresh conversion is available. Original fields from the last report are shown below.':profile?`SEN0219 v1 · ${((gas.assumed_reference_mv || snapshot.co2_settings.reference_mv)/1000).toFixed(3)} V reference. Counts and ppm use different units.`:'Identify the module and select its profile to convert voltage into ppm.');
    }
    renderReportAge();
    const signature=JSON.stringify(sensor.wires);
    if($('wire-paths').dataset.wires!==signature) {
      $('wire-paths').dataset.wires=signature;
      $('wire-paths').replaceChildren(...sensor.wires.map(([pin,end,kind])=>{
        const row=document.createElement('div');row.className=`wire-path wire-${kind}`;
        for(const [value,cls] of [[pin,'wire-pin'],['','wire-line'],[end,'wire-end']]) {const e=document.createElement('span');e.className=cls;e.textContent=value;row.append(e);}return row;
      }));
    }
    document.querySelectorAll('[data-trace],[data-pin-sensor]').forEach(b=>b.setAttribute('aria-pressed',String((b.dataset.trace || b.dataset.pinSensor)===traced)));
    document.querySelectorAll('[data-trace]').forEach(b=>{
      if(narrowInspector.matches)b.setAttribute('aria-expanded',String(b.dataset.trace===traced && !$('wiring-inspector').hidden));
      else b.removeAttribute('aria-expanded');
    });
    document.querySelectorAll('[data-sensor]').forEach(e=>e.classList.toggle('is-traced',e.dataset.sensor===traced));
    placeInspector();
  }
  function downloadSensor() {
    const sensor=sensorRows.find(s=>s.id===traced),device=selectedDevice();
    if(!sensor || !device)return;
    const fields=Object.fromEntries((sensorFields[sensor.id] || []).filter(key=>device.fields[key]!==undefined).map(key=>[key,device.fields[key]]));
    if(!Object.keys(fields).length)return;
    const record={
      format:'aog-sensor-snapshot-v1',device:device.device_id,port:device.port,
      sensor:sensor.id,pins:sensor.pin,firmware:device.fields.FIRMWARE_VERSION || null,
      reported_at:device.observed_at?new Date(device.observed_at*1000).toISOString():null,
      live:fresh(device),display_value:fresh(device)?sensor.value || null:null,
      status:fresh(device)?sensor.status[0]:'Unavailable',fields
    };
    if(sensor.id==='analog')record.co2_profile={...snapshot.co2_settings};
    const url=URL.createObjectURL(new Blob([JSON.stringify(record,null,2)+'\n'],{type:'application/json'}));
    const link=document.createElement('a');link.href=url;link.download=`aog-${selected.toLowerCase()}-${sensor.id}-${device.observed_at || 'unknown'}.json`;
    document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
  function renderPhInput() {
    const device=snapshot?.devices.find(device=>device.device_id==='DUAL_OVF_SENSOR');
    const enabled=Boolean(snapshot?.ph_input?.connected),live=fresh(device),fields=device?.fields || {};
    set('ph-tool-summary',enabled?'Physical wiring confirmed · analog connection cannot be auto-detected':'Not enabled · raw voltage is excluded from water pH');
    set('ph-input-state',enabled?'Atlas Gravity input enabled. Check with known pH buffers before relying on its measurements.':'Awaiting your physical connection check. No unverified pH values will be recorded.');
    set('ph-input-adc',live && fields.PH_ADC!==undefined?`${fields.PH_ADC} counts`:'Unavailable');
    set('ph-input-voltage',live && numeric(fields.PH_MV)!==null?`${(Number(fields.PH_MV)/1000).toFixed(3)} V`:'Unavailable');
    $('ph-confirm-wrap').hidden=enabled;$('ph-input-enable').hidden=enabled;$('ph-input-disable').hidden=!enabled;
    $('ph-input-confirm').disabled=phSaving || enabled;
    $('ph-input-enable').disabled=phSaving || !$('ph-input-confirm').checked;
    $('ph-input-disable').disabled=phSaving;
  }
  async function savePhInput(connected) {
    if(phSaving)return;
    phSaving=true;renderPhInput();
    try {
      await api('/api/ph-input',{body:{connected}});
      $('ph-input-confirm').checked=false;
      set('ph-input-result',connected?'Connection confirmed. Previous calibration was archived; calibrate again using known buffers. Earlier unverified pH records remain excluded.':'pH input disabled. Raw electrical diagnostics remain available.');
      await refresh();
    }catch(error){set('ph-input-result',error.message);}
    finally{phSaving=false;renderPhInput();}
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
    set('co2-tool-summary',primary?'Dashboard source: DFRobot infrared CO₂':d?.source==='ccs811'?'Dashboard source: CCS811 equivalent CO₂':d?.source==='legacy_mixed'?'Legacy mixed CO₂ · upgrade firmware to separate the sources':'CO₂ source not confirmed');
    $('gas-analog-primary').closest('.gas-comparison').classList.toggle('infrared-primary',primary);
    tag('gas-analog-status',adc==null?'Unavailable':bad?'Check signal voltage':configured?(primary?'Used for CO₂':'Converted reading'):'Model unconfirmed',adc==null?'neutral':bad?'warning':configured?'good':'neutral');
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
    lastFreshness=snapshot.devices.map(d=>String(fresh(d))).join();
    const device=selectedDevice(),connected=fresh(device),f=device?.fields || {},kit=selected==='SENSORKIT_MK1';
    const good=snapshot.devices.filter(fresh).length;
    set('hardware-summary',`${good} of 2 Arduino links reporting · select a board, then a sensor.`);
    set('hardware-updated',`Checked ${new Date(snapshot.server_time*1000).toLocaleTimeString()}`);
    snapshot.devices.forEach(d=>{
      const short=d.device_id==='SENSORKIT_MK1'?'kit':'ovf', rows=sensorsFor(d), warnings=rows.filter(s=>s.status[1]==='warning').length;
      $('map-'+short).dataset.status=nodeStatus(d);
      set('map-status-'+short,connectionFailed?'○ Status unavailable':fresh(d)?'● USB reporting':d.observed_at?'○ Offline':'○ Not identified');
      set('map-port-'+short,d.port || 'Discovering USB…');
      const reporting=rows.filter(s=>s.status[1]==='good').length;
      set('map-health-'+short,!fresh(d)?'Sensor readings unavailable':warnings?`${warnings} sensor${warnings>1?'s':''} need attention`:`${reporting} reporting · ${rows.length-reporting} unverified / optional`);
      $('map-health-'+short).dataset.warning=String(warnings>0 && fresh(d));
    });
    set('inspector-title',`${names[selected]} connections`);set('board-title',names[selected]);
    tag('board-status',connectionFailed?'Status unavailable':connected?'USB reporting':device?.observed_at?'Offline':'Not identified',connected?'good':connectionFailed?'neutral':'warning');
    set('board-port',device?.port || 'Waiting for serial discovery');set('board-seen',stamp(device?.observed_at));
    set('board-version',f.FIRMWARE_VERSION?`v${f.FIRMWARE_VERSION} · protocol ${f.PROTOCOL_VERSION || 'legacy'}`:'Unknown');
    set('board-buffer',f.PROTOCOL_VERSION==='2'?`${f.BUFFER_PENDING || 0} / ${f.BUFFER_CAPACITY || '—'} readings`:'Requires firmware v2');
    set('board-dropped',f.PROTOCOL_VERSION==='2'?`${f.BUFFER_DROPPED || 0} since board reset`:'Not reported by this firmware');
    set('board-clock',f.PROTOCOL_VERSION==='2'?`${f.UPTIME_S || 0}s since board boot`:'No device clock report');
    const dropped=numeric(f.BUFFER_DROPPED)||0,pending=numeric(f.BUFFER_PENDING)||0,capacity=numeric(f.BUFFER_CAPACITY)||24;
    set('board-detail-summary',!connected?'Last received report · board currently unavailable':`${device.port || 'USB'} · v${f.FIRMWARE_VERSION || '?'} · ${f.PROTOCOL_VERSION!=='2'?'Buffered logs require v2':dropped?`${dropped} dropped records`:pending?`${pending} buffered readings`:'Logs saved'}`);
    $('board-details').dataset.status=!connected || dropped?'warning':'neutral';
    set('board-log-health',!connected?'No current buffer report. Last received details are shown above.':f.PROTOCOL_VERSION!=='2'?'Install firmware v2 for buffered logging.':dropped?`${dropped} readings were dropped since reset. ${pending} waiting for the Pi.`:pending?`${pending} readings waiting to be saved by the Pi.`:'All buffered readings saved. No dropped records.');
    $('board-buffer-level').style.width=`${connected?Math.min(100,pending/capacity*100):0}%`;
    set('board-errors',device?.error || '');$('board-errors').hidden=!device?.error;
    $('board-connection-help').hidden=connected;
    set('board-connection-help',connectionFailed?'AOG could not refresh connection details. Retry Refresh connections to check the current board status.':'Waiting for a fresh board report. Check its USB connection and power, then refresh connections.');
    sensorRows=sensorsFor(device);
    if(!sensorRows.some(s=>s.id===traced))traced=sensorRows.find(s=>s.status[1]==='warning')?.id || (kit && snapshot.co2_settings?.primary_source==='analog'?'analog':sensorRows[0].id);
    renderSensors();
    renderWiring();
    renderChecks();
    set('raw-device-frame',Object.entries(f).map(([k,v])=>`${k}: ${v}`).join('\n') || 'A raw report will appear when this Arduino is identified.');
    set('pin-a0-label',kit?'Gas module signal':'pH interface signal');set('pin-d2-label',kit?'Unused':'Overflow switch 1');set('pin-d4-label',kit?'Unused':'Overflow switch 2');set('pin-d7-label',kit?'DHT11 data':'Unused');set('pin-a4-label',kit?'CCS811 SDA':'Unused');set('pin-a5-label',kit?'CCS811 SCL':'Unused');
    if(!dirty)snapshot.water_probes.forEach((probe,i)=>{$(`water-label-${i}`).value=probe.label;$(`water-source-${i}`).value=probe.source || '';});
    snapshot.water_readings.forEach((value,i)=>{
      const age=value?reportAge({observed_at:value[1]}):null;
      set(`water-live-${i}`,!connectionFailed && age!==null && age>=0 && age<=5?`${value[0].toFixed(2)} °C`:'No unambiguous reading');
    });
    set('water-save',saving?'Saving…':dirty?'Save probe names & sources':'Probe settings saved');$('water-save').disabled=saving || !dirty;
    renderGas();
    renderPhInput();renderFirmware();
  }
  function renderFirmware() {
    if (!firmware) {
      $('firmware-install').disabled=true;
      $('firmware-hex').hidden=true;
      $('firmware-source').hidden=true;
      set('firmware-tool','Firmware details could not be refreshed. Try refreshing connections.');
      set('firmware-tool-summary','Firmware details unavailable · refresh to retry');
      return;
    }
    set('firmware-version', firmware.package ? `v${firmware.package.version}` : 'Package unavailable');
    set('firmware-tool', firmware.updater_available ? 'Upload tools installed' : 'Upload tools are not installed on this Pi');
    const job = firmware.job;
    set('firmware-tool-summary',firmware.active?`Update in progress · ${names[job?.device] || names[selected]}`:firmware.package?`Packaged v${firmware.package.version} · sources, binaries & installation`:'Firmware package unavailable');
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
  async function refresh(manual=false) {
    if (!api || busy) return;
    busy=true;
    if(manual) {$('hardware-refresh').disabled=true;set('hardware-refresh','Refreshing…');}
    try {
      const [connectionResult, firmwareResult] = await Promise.allSettled([api('/api/connections').then(data=>({data,receivedAt:performance.now()})), api('/api/firmware')]);
      if(connectionResult.status==='rejected')throw connectionResult.reason;
      const data=connectionResult.value.data,fw=firmwareResult.status==='fulfilled'?firmwareResult.value:null;
      if (!Number.isFinite(data.server_time) || !Array.isArray(data.devices) || data.devices.length!==2 || new Set(data.devices.map(d=>d.device_id)).size!==2 || data.devices.some(d=>!names[d.device_id] || !Number.isFinite(d.observed_at) || !d.fields || typeof d.fields!=='object') || !Array.isArray(data.water_probes) || !Array.isArray(data.water_readings)) throw new Error('Invalid connection report');
      snapshot=data;receivedAt=connectionResult.value.receivedAt;connectionFailed=false;firmware=fw;render();$('hardware-error').hidden=true;
    } catch(error) {
      set('hardware-error', `Connection details could not be refreshed. ${error.message}`);$('hardware-error').hidden=false;
      // Invalidate old green states immediately when the Pi cannot be reached.
      connectionFailed=true;
      if (snapshot)render();
    } finally {
      busy=false;
      if(manual) {$('hardware-refresh').disabled=false;set('hardware-refresh','↻ Refresh connections');}
    }
  }
  function init(client) {
    api=client;if(initialized) return;initialized=true;
    document.querySelectorAll('[data-board]').forEach(button => button.addEventListener('click', () => select(button.dataset.board)));
    document.querySelectorAll('[data-sensor-filter]').forEach(button=>button.addEventListener('click',()=>{
      sensorFilter=button.dataset.sensorFilter;
      if(sensorFilter==='attention') {
        const warnings=sensorRows.filter(s=>s.status[1]==='warning');
        if(!warnings.some(s=>s.id===traced) && warnings.length)traced=warnings[0].id;
      }
      renderFilter();renderWiring();
      set('connection-announcement',sensorFilter==='all'?`Showing all ${sensorRows.length} sensors.`:`${sensorRows.filter(s=>s.status[1]==='warning').length} sensors need attention.`);
    }));
    $('page-connections').addEventListener('click',event=>{
      const trigger=event.target.closest('[data-hardware-jump]');
      if(trigger) {event.preventDefault();openTool(trigger.dataset.hardwareJump);}
    });
    $('trace-back').addEventListener('click',backToSensors);
    document.querySelectorAll('[data-inspection-view]').forEach(tab=>{
      tab.addEventListener('click',()=>setInspectionView(tab.dataset.inspectionView));
      tab.addEventListener('keydown',event=>{
        if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
        event.preventDefault();
        const view=event.key==='Home'?'wiring':event.key==='End'?'data':inspectionView==='wiring'?'data':'wiring';
        setInspectionView(view,true);
      });
    });
    $('trace-download').addEventListener('click',downloadSensor);
    narrowInspector.addEventListener('change',()=>{
      if(narrowInspector.matches && $('wiring-inspector').contains(document.activeElement))mobileInspectorOpen=true;
      renderFilter();renderWiring();
    });
    $('hardware-refresh').addEventListener('click',()=>refresh(true));
    $('firmware-board').addEventListener('change',()=>{updateError='';renderFirmware();});
    window.addEventListener('beforeunload',event=>{if(dirty || co2Dirty){event.preventDefault();event.returnValue='';}});
    $('ph-input-confirm').addEventListener('change',renderPhInput);
    $('ph-input-form').addEventListener('submit',event=>{event.preventDefault();if($('ph-input-confirm').checked)savePhInput(true);});
    $('ph-input-disable').addEventListener('click',()=>savePhInput(false));
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
    setInterval(()=>{
      if(document.hidden || location.hash!=='#connections' || !snapshot)return;
      if(lastFreshness!==snapshot.devices.map(d=>String(fresh(d))).join())render();
      else renderReportAge();
    },1000);
    select(selected);
    if(location.hash==='#connections') refresh();
  }
  return {init,refresh};
})();
