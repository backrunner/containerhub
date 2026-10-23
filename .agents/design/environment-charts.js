// Rolling synthetic samples; updates are scoped to visible environment metrics.
const environmentSamples = new Map();
let environmentTick = 0;
function telemetrySamples(t, key) {
  const id=t.id+':'+key;
  if(!environmentSamples.has(id)) {
    const base=t[key], now=Date.now();
    environmentSamples.set(id,Array.from({length:151},(_,i)=>({at:now-(150-i)*2000,value:sampleValue(base,i,t.id.length+key.length,key)})));
  }
  return environmentSamples.get(id);
}
function sampleValue(base,i,seed,key) {
  const wave=Math.sin((i+seed)*0.36)*.15+Math.sin(i*.91+seed)*.06+Math.cos(i*.12)*.08;
  return Math.max(.02,base*(1+wave*(key==='mem'?.14:1)));
}
function chartPath(values, max) {
  const end=values.at(-1).at,start=end-environmentRange*1000;
  return values.map((v,i)=>`${i?'L':'M'}${Math.max(0,(v.at-start)/(end-start)*360).toFixed(1)} ${(64-Math.min(v.value/max,1)*56).toFixed(1)}`).join(' ');
}
function metricText(t,key) {
  const samples=telemetrySamples(t,key), value=samples.at(-1).value;
  if(key==='cpu') return `${value.toFixed(1)}<small>%</small>`;
  if(key==='mem') return `${value.toFixed(1)}<small>/ ${t.memory} GiB</small>`;
  if(key==='network') return `${value.toFixed(2)}<small>MB/s</small>`;
  return `${t.diskUsed}<small>/ ${t.disk} GiB</small>`;
}
function metricChart(t,key) {
  const all=telemetrySamples(t,key), values=all.filter(v=>v.at>=all.at(-1).at-environmentRange*1000);
  const max=key==='cpu'?100:key==='mem'?t.memory:Math.max(...all.map(v=>v.value))*1.3;
  return `<svg viewBox="0 0 360 72" preserveAspectRatio="none" role="img" aria-label="${{cpu:'CPU',mem:'内存',network:'网络吞吐',io:'磁盘读写'}[key]}${environmentRange===60?'最近一分钟':'最近五分钟'}示例趋势"><path class="chart-grid" d="M0 8H360M0 36H360M0 64H360"/><path class="chart-line" d="${chartPath(values,max)}"/>${key==='network'?`<path class="chart-line secondary" d="${chartPath(values.map((v,i)=>({...v,value:v.value*(.22+Math.sin(i*.4)*.06)})),max)}"/>`:''}</svg>`;
}
function renderEnvironmentMetrics(t) {
  const offline=environmentIsOffline(t);
  const specs=[['cpu','Cpu','CPU','整机 · 100%'],['mem','Layers','内存','主机物理内存'],['network','Network','网络',`<span class="chart-key"></span>接收 <span class="chart-key secondary"></span>发送`],['io','Database','磁盘',`已用 ${Math.round(t.diskUsed/t.disk*100)}% · 读写 <span data-io-rate>${telemetrySamples(t,'io').at(-1).value.toFixed(1)}</span> MB/s`]];
  return `<section class="env-section telemetry-section" aria-labelledby="host-metrics-title"><div class="env-section-heading"><h3 id="host-metrics-title">主机资源</h3><div class="telemetry-controls"><span class="telemetry-source" data-sample-label>${offline?'历史采样':environmentPaused?'已暂停':'示例遥测 · 每 '+t.refresh+' 秒'}</span><div class="range-control" role="group" aria-label="图表时间范围"><button data-metric-range="60" aria-pressed="${environmentRange===60}">1 分钟</button><button data-metric-range="300" aria-pressed="${environmentRange===300}">5 分钟</button></div><button class="icon-button bordered" id="telemetry-pause" aria-label="${environmentPaused?'继续示例遥测':'暂停示例遥测'}" ${offline?'disabled':''}>${icon(environmentPaused?'Play':'Pause')}</button></div></div><div class="env-metrics ${offline?'stale':''}">${specs.map(([key,glyph,label,caption])=>`<article class="host-metric" data-metric="${key}"><div class="host-metric-label">${icon(glyph)}${label}</div><div class="host-metric-value tabular" data-metric-value="${key}">${metricText(t,key)}</div><div class="host-metric-caption">${caption}</div><div class="host-chart" data-chart="${key}">${metricChart(t,key)}</div><div class="chart-axis"><span>${environmentRange===60?'60 秒前':'5 分钟前'}</span><span>${offline?'断开时':environmentPaused?'暂停时':'现在'}</span></div></article>`).join('')}</div></section>`;
}
function bindEnvironmentCharts() {
  document.querySelectorAll('[data-metric-range]').forEach(b=>b.onclick=()=>{environmentRange=Number(b.dataset.metricRange);renderEnvironmentDetail();document.querySelector(`[data-metric-range="${environmentRange}"]`)?.focus();});
  document.querySelector('#telemetry-pause')?.addEventListener('click',()=>{environmentPaused=!environmentPaused;renderEnvironmentDetail();document.querySelector('#telemetry-pause')?.focus();});
}
setInterval(()=>{
  if(typeof state==='undefined'||state.page!=='runtime'||document.hidden||environmentPaused||document.querySelector('#modal')?.open) return;
  const t=inspectedTarget(); if(environmentIsOffline(t)) return;
  const now=Date.now();if(now-(t.sampledAt||0)<t.refresh*1000)return;t.sampledAt=now;
  environmentTick++;
  for(const key of ['cpu','mem','network','io']) {
    const values=telemetrySamples(t,key);values.push({at:now,value:sampleValue(t[key],150+environmentTick,t.id.length+key.length,key)});while(values.length>1&&values[0].at<now-300000)values.shift();
    const figure=document.querySelector(`[data-metric-value="${key}"]`),chart=document.querySelector(`[data-chart="${key}"]`);
    if(figure)figure.innerHTML=metricText(t,key);
    if(chart)chart.innerHTML=metricChart(t,key);
  }
  const io=document.querySelector('[data-io-rate]');if(io)io.textContent=telemetrySamples(t,'io').at(-1).value.toFixed(1);
  const beat=document.querySelector('[data-heartbeat]');if(beat)beat.textContent='刚刚';
},1000);
