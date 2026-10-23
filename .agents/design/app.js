if(new URLSearchParams(location.search).has('preview')) document.documentElement.dataset.preview='true';
/* ContainerHub design prototype. All operations are local simulations. */
'use strict';
const $ = (s, root = document) => root.querySelector(s);
const $$ = (s, root = document) => [...root.querySelectorAll(s)];
const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const icon = (name, cls = '') => `<svg class="icon ${cls}" aria-hidden="true" viewBox="0 0 24 24">${(window.CH_ICONS[name] || window.CH_ICONS.Box).map(([tag, attrs]) => `<${tag} ${Object.entries(attrs).map(([k,v]) => `${k}="${esc(v)}"`).join(' ')}></${tag}>`).join('')}</svg>`;
function hydrate(root = document) { $$('[data-icon]', root).forEach(el => { el.outerHTML = icon(el.dataset.icon, el.className === 'brand-icon' ? '' : ''); }); }
// Product branding uses the generated v4 isometric icon; Lucide is reserved for interface actions.
hydrate();
const seedContainers = [
  {id:'web-api', image:'ghcr.io/acme/api:dev', status:'running', cpu:2.4, memory:186, port:3000, target:3000, uptime:'2 小时', volume:'api-data', project:'platform'},
  {id:'postgres', image:'postgres:17-alpine', status:'running', cpu:0.8, memory:352, port:5432, target:5432, uptime:'6 小时', volume:'postgres-data', project:'platform'},
  {id:'redis', image:'redis:7-alpine', status:'running', cpu:0.2, memory:28, port:6379, target:6379, uptime:'6 小时', volume:'redis-data', project:'platform'},
  {id:'minio', image:'minio/minio:latest', status:'running', cpu:1.1, memory:294, port:9000, target:9000, uptime:'3 小时', volume:'minio-data', project:'storage'},
  {id:'nginx-proxy', image:'nginx:1.27-alpine', status:'running', cpu:0.1, memory:18, port:8080, target:80, uptime:'6 小时', volume:null, project:'platform'},
  {id:'worker', image:'ghcr.io/acme/api:dev', status:'stopped', cpu:0, memory:0, port:null, target:null, uptime:'—', volume:null, project:'platform'},
  {id:'sandbox', image:'alpine:3.22', status:'stopped', cpu:0, memory:0, port:null, target:null, uptime:'—', volume:null, project:'playground'},
  {id:'mailpit', image:'axllent/mailpit:latest', status:'error', cpu:0, memory:0, port:8025, target:8025, uptime:'—', volume:null, project:'platform'}
];
const seedImages = [
  {name:'ghcr.io/acme/api',tag:'dev',size:'248 MB',used:2},
  {name:'postgres',tag:'17-alpine',size:'292 MB',used:1},
  {name:'redis',tag:'7-alpine',size:'42 MB',used:1},
  {name:'minio/minio',tag:'latest',size:'163 MB',used:1},
  {name:'nginx',tag:'1.27-alpine',size:'48 MB',used:1},
  {name:'alpine',tag:'3.22',size:'8.2 MB',used:1},
  {name:'axllent/mailpit',tag:'latest',size:'34 MB',used:1}
];
let containers = structuredClone(seedContainers);
let images = structuredClone(seedImages);
let volumes = [
  {name:'api-data',size:'128 MB',used:'web-api',mount:'/app/data'},
  {name:'postgres-data',size:'1.4 GB',used:'postgres',mount:'/var/lib/postgresql/data'},
  {name:'redis-data',size:'8 MB',used:'redis',mount:'/data'},
  {name:'minio-data',size:'2.1 GB',used:'minio',mount:'/data'},
  {name:'test-cache',size:'64 MB',used:null,mount:'—'}
];
let networks = [{name:'default',subnet:'192.168.64.0/24',count:5},{name:'platform-dev',subnet:'192.168.65.0/24',count:3}];
let tasks = [
  {id:3,title:'启动 web-api',target:'web-api',action:'start',status:'success',detail:'容器已启动，状态已确认',time:'2 分钟前'},
  {id:2,title:'拉取 ghcr.io/acme/api:dev',status:'success',detail:'镜像已就绪 · linux/arm64',time:'3 分钟前'},
  {id:1,title:'启动 mailpit',target:'mailpit',action:'start',status:'error',detail:'端口 8025 被占用，请修改映射后重试',time:'12 分钟前'}
];
const state = {containerPage:0,pageSize:10,page:'containers',selected:'web-api',tab:'overview',filter:'all',query:'',checked:new Set(),scenario:'normal',logFollow:true,taskFilter:'all',generation:0};
let toastTimer, taskId = 4, modalTrigger;
const pageInfo = {
  containers:['容器','当前用户的本机容器','新建容器','拉取镜像'],
  overview:['概览','运行状态、资源用量与异常','新建容器','刷新'],
  images:['镜像','本地镜像、标签与容器引用','新建容器','拉取镜像'],
  volumes:['存储卷','持久数据、挂载路径与引用关系','新建存储卷','空间概况'],
  networks:['网络','容器连接与地址空间','新建网络','连接概况'],
  stacks:['Stacks','','刷新','刷新'],
  runtime:['环境管理','本机与已绑定的远程环境','添加远程环境','刷新'],
  tasks:['任务中心','操作进度、结果与失败重试','刷新','清除筛选']
};
function isOffline() { return state.scenario === 'offline'; }
function resource(id) { return containers.find(c => c.id === id); }
function statusPill(status) {
  const labels = {running:'运行中',stopped:'已停止',error:'异常',starting:'启动中',stopping:'停止中',restarting:'重启中'};
  const busy = ['starting','stopping','restarting'].includes(status);
  return `<span class="status-pill ${busy ? 'busy' : status}">${busy ? icon('LoaderCircle','spin') : `<i class="status-dot ${status === 'running' ? 'online' : status === 'error' ? 'red' : ''}"></i>`}${labels[status] || '未知'}</span>`;
}
function spark(variant = 0) {
  const paths = ['M0 20L8 19L15 21L22 11L29 14L36 12L43 16L50 8L57 10L64 6L72 10L80 7L88 9L96 4L104 7L112 5', 'M0 22L8 21L16 19L24 20L32 16L40 16L48 13L56 15L64 11L72 12L80 8L88 9L96 8L104 6L112 5'];
  return `<svg class="mini-spark" viewBox="0 0 112 28" preserveAspectRatio="none" role="img" aria-label="示例用量趋势"><path d="${paths[variant % 2]}" stroke="currentColor" stroke-width="1.6" fill="none"/></svg>`;
}
function memoryText(mib) { return mib >= 1024 ? `${(mib / 1024).toFixed(2)} GiB` : `${mib} MiB`; }
function summary() {
  const running = containers.filter(c => c.status === 'running');
  const stopped = containers.filter(c => c.status === 'stopped').length;
  const errors = containers.filter(c => c.status === 'error').length;
  return `<div class="summary-strip" aria-label="容器状态摘要">
    <div class="summary-item"><div class="summary-label">运行中</div><div class="summary-value">${running.length}<span>/ ${containers.length} 个容器</span></div></div>
    <div class="summary-item"><div class="summary-label">已停止</div><div class="summary-value">${stopped}<span>${errors ? `<span class="summary-error">${errors} 个异常</span>` : ''}</span></div></div>
    <div class="summary-item"><div class="summary-label">CPU <span>（单核 100%）</span></div><div class="summary-value">${running.reduce((s,c)=>s+c.cpu,0).toFixed(1)}<span>%</span></div></div>
    <div class="summary-item"><div class="summary-label">内存用量</div><div class="summary-value">${(running.reduce((s,c)=>s+c.memory,0)/1024).toFixed(2)}<span>GiB</span></div></div>
  </div>`;
}
function visibleContainers() {
  return containers.filter(c => (state.filter === 'all' || c.status === state.filter) && `${c.id} ${c.image} ${c.project}`.toLowerCase().includes(state.query.toLowerCase()));
}
function render() {
  const [title, description, primary, secondary] = pageInfo[state.page];
  $('#page-title').textContent = title;
  $('#breadcrumb-current').textContent = title;
  $('#page-description').textContent = description;
  $('#page-description').hidden = state.page === 'containers';
  $('#page-count').textContent = ({containers:containers.length,images:images.length,volumes:volumes.length,networks:networks.length,tasks:tasks.length})[state.page] ?? '';
  $('#page-count').hidden = ['overview','runtime'].includes(state.page);
  $('#primary-action').innerHTML = icon(['containers','overview','images','volumes','networks'].includes(state.page) ? 'Plus' : state.page === 'tasks' ? 'RefreshCw' : 'Stethoscope') + esc(primary);
  $('#secondary-action').innerHTML = icon(['containers','images'].includes(state.page) ? 'Download' : 'SlidersHorizontal') + esc(secondary);
  $('#primary-action').disabled = isOffline() && ['containers','overview','images','volumes','networks'].includes(state.page);
  $('#secondary-action').disabled = isOffline() && ['containers','images'].includes(state.page);
  $('#container-count').textContent = containers.length;
  $('#image-count').textContent = images.length;
  $('#task-count').textContent = tasks.filter(t=>t.status==='error'||t.status==='busy').length || tasks.length;
  $('#offline-banner').hidden = !isOffline() || state.page==='runtime';
  $$('#navigation [data-page]').forEach(b=>{b.classList.toggle('active',b.dataset.page===state.page);if(b.dataset.page===state.page)b.setAttribute('aria-current','page');else b.removeAttribute('aria-current');});
  $('#page-content').dataset.page=state.page;
  $('#page-content').innerHTML = ({containers:renderContainers,overview:renderOverview,images:renderImages,volumes:renderVolumes,networks:renderNetworks,runtime:renderRuntime,stacks:renderStacks,tasks:renderTasks})[state.page]();
  if(state.page==='runtime'){ $('#primary-action').innerHTML=icon('Plus')+'添加环境'; $('#primary-action').className='button'; $('#primary-action').style.order='0'; $('#secondary-action').style.order='1'; $('#secondary-action').className='icon-button bordered'; $('#secondary-action').innerHTML=icon('RefreshCw'); $('#secondary-action').setAttribute('aria-label','刷新环境'); } else { $('#primary-action').className='button primary'; $('#primary-action').style.order=''; $('#secondary-action').style.order=''; $('#secondary-action').className='button'; $('#secondary-action').removeAttribute('aria-label'); }
  document.querySelector('[data-page=stacks]').hidden=envTarget(reviewEnvironment)?.provider!=='docker';
  if(state.page==='stacks'){ $('#page-description').hidden=true;$('#page-count').hidden=true;$('#primary-action').hidden=true;$('#secondary-action').className='icon-button bordered';$('#secondary-action').innerHTML=icon('RefreshCw');$('#secondary-action').setAttribute('aria-label','刷新 Stack');}else{$('#primary-action').hidden=false;}
  bindStacks();
  bindPage();
  bindEnvironment();
  bindImages();
  bindPagination();
  $$('[data-edit-ports]').forEach(b=>b.onclick=()=>portEditor(b.dataset.editPorts));
}
function renderContainers() {
  const {items:visible,total} = containerPage();
  const current = resource(state.selected);
  return `${summary()}<div class="content-split" ${!current ? 'style="grid-template-columns:1fr"' : ''}><div class="list-area">
    <div class="list-toolbar"><label class="search-field">${icon('Search')}<input id="resource-search" placeholder="搜索名称、镜像或项目…" aria-label="搜索容器" value="${esc(state.query)}"></label><div class="toolbar-end"><select class="filter-select" id="status-filter" aria-label="容器状态"><option value="all">全部状态</option><option value="running">运行中</option><option value="stopped">已停止</option><option value="error">异常</option></select><button class="icon-button refresh-button" aria-label="刷新容器">${icon('RefreshCw')}</button></div></div>
    ${state.checked.size ? `<div class="batch-bar"><strong>已选择 ${state.checked.size} 个容器</strong><button class="button small" data-batch="start" ${isOffline()?'disabled':''}>${icon('Play')}启动</button><button class="button small danger" data-batch="stop" ${isOffline()?'disabled':''}>${icon('Square')}停止</button><button class="button small danger" data-batch="delete" ${isOffline()?'disabled':''}>${icon('Trash2')}删除</button><button class="icon-button" id="clear-selection" aria-label="取消选择">${icon('X')}</button></div>` : ''}
    <div class="table-wrap">${visible.length ? `<table aria-label="容器列表"><thead><tr><th class="check-cell"><input type="checkbox" id="select-all" aria-label="选择当前页的全部容器" ${visible.every(c=>state.checked.has(c.id))?'checked':''}></th><th><div class="sort-heading"><span>容器</span><span class="sort-controls"><button class="icon-button" id="sort-field" aria-label="切换排序字段">${icon('ArrowUpDown')}</button><button class="icon-button" id="sort-direction" aria-label="切换排序方向">${icon('ArrowUp')}</button></span></div></th><th class="container-state-col">状态</th><th class="container-cpu-col">CPU</th><th class="container-memory-col">内存</th><th class="port-col">端口</th><th class="container-actions-col">操作</th></tr></thead><tbody>${visible.map(c=>`<tr class="${state.selected===c.id?'selected':''}">
    <td class="check-cell"><input type="checkbox" data-check="${esc(c.id)}" aria-label="选择 ${esc(c.id)}" ${state.checked.has(c.id)?'checked':''}></td><td><button class="resource-name" data-inspect="${esc(c.id)}" title="${esc(c.id)}">${esc(c.id)}</button><span class="resource-sub" title="${esc(c.image)}">${esc(c.image)}</span></td><td class="container-state-col">${statusPill(c.status)}</td><td class="numeric container-cpu-col">${c.status==='running'?c.cpu.toFixed(1)+'%':'—'}</td><td class="numeric container-memory-col">${c.status==='running'?memoryText(c.memory):'—'}</td><td class="port-col">${c.port?`<button class="port-button" data-copy="127.0.0.1:${c.port}" aria-label="复制 ${esc(c.id)} 端口地址">${c.port} → ${c.target}</button>`:'<span class="muted">—</span>'}</td><td><div class="row-actions"><button class="lifecycle-action ${c.status!=='running'?'start':''}" data-life="${c.status==='running'?'stop':'start'}" data-id="${esc(c.id)}" aria-label="${c.status==='running'?'停止':'启动'} ${esc(c.id)}" ${isOffline()||['starting','stopping','restarting'].includes(c.status)?'disabled':''}>${icon(c.status==='running'?'Square':'Play')}<span>${c.status==='running'?'停止':'启动'}</span></button><button class="icon-button restart-shortcut" data-life="restart" data-id="${esc(c.id)}" aria-label="重启 ${esc(c.id)}" ${isOffline()||c.status!=='running'?'disabled':''}>${icon('RotateCw')}</button><button class="icon-button" data-more="${esc(c.id)}" aria-label="${esc(c.id)} 更多操作">${icon('Ellipsis')}</button></div></td></tr>`).join('')}</tbody></table>` : `<div class="empty-state">${icon(containers.length?'SearchX':'Container')}<h2>${containers.length?'没有匹配的容器':'从第一个容器开始'}</h2><p>${containers.length?'尝试其他关键词，或清除筛选条件。':'选择一个镜像，为你的应用准备一个独立环境。'}</p><button class="button primary" id="empty-action">${icon(containers.length?'FilterX':'Plus')}${containers.length?'清除筛选':'新建容器'}</button></div>`}</div>
    ${renderPagination(total)}</div>${current?`<div class="inspector-slot">${renderInspector(current)}</div>`:''}</div>`;
}
function renderRecent() {
  return `<div class="recent-activity"><div class="recent-title"><span>最近操作</span><button data-nav="tasks">查看全部 ${icon('ArrowRight')}</button></div>${tasks.slice(0,3).map(t=>`<div class="recent-row ${t.status==='error'?'error':''}">${icon(t.status==='error'?'CircleAlert':t.status==='busy'?'LoaderCircle':'CircleCheck')}<span><strong>${esc(t.title)}</strong> · ${t.status==='success'?'已完成':t.status==='busy'?'进行中':'失败'}</span><time>${esc(t.time)}</time></div>`).join('')}</div>`;
}
function collapseInspector() {
  const panel = $('.inspector'), slot = $('.inspector-slot'), split = $('.content-split');
  if (!panel || slot.dataset.closing) return;
  const id = state.selected;
  const finish = () => {
    if (!slot.isConnected || state.selected !== id) return;
    state.selected = null;
    render();
    $$('[data-inspect]').find(button => button.dataset.inspect === id)?.focus({preventScroll:true});
  };
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) { finish(); return; }
  slot.dataset.closing = 'true';
  slot.inert = true;
  const options = {duration:200, easing:'cubic-bezier(.333333,0,.666667,1)', fill:'forwards'};
  const animations = [slot.animate([{opacity:1},{opacity:0}], options)];
  if (getComputedStyle($('.list-area')).display !== 'none') {
    const width = slot.getBoundingClientRect().width;
    panel.style.width = `${width}px`;
    animations.push(split.animate([
      {gridTemplateColumns:`minmax(0,1fr) ${width}px`, columnGap:getComputedStyle(split).columnGap},
      {gridTemplateColumns:'minmax(0,1fr) 0px', columnGap:'0px'}
    ], options));
  }
  Promise.all(animations.map(animation => animation.finished)).then(finish).catch(()=>{});
}
function renderInspector(c) {
  const tabs = [['overview','概览'],['logs','日志'],['config','配置'],['terminal','终端']];
  return `<aside class="inspector" aria-label="${esc(c.id)} 详情"><div class="inspector-heading"><div class="inspector-heading-top"><span class="container-glyph">${icon('Box')}</span><div><h2>${esc(c.id)}</h2><div class="image-line">${esc(c.image)}</div></div><button class="icon-button bordered" id="close-inspector" aria-label="收起详情" title="收起详情">${icon('PanelRightClose')}</button></div><div class="inspector-status">${statusPill(c.status)}<span>${c.status==='running'?`运行 ${c.uptime}`:'等待启动'}</span></div></div><div class="inspector-tabs" role="tablist" aria-label="容器详情标签">${tabs.map(([key,label])=>`<button role="tab" id="tab-${key}" data-tab="${key}" aria-selected="${state.tab===key}" aria-controls="inspector-panel" ${key==='terminal'?'disabled title="当前运行时尚不支持终端会话"':''}>${label}</button>`).join('')}</div><div class="inspector-body" id="inspector-panel" role="tabpanel" aria-labelledby="tab-${state.tab}">${({overview:inspectorOverview,logs:inspectorLogs,config:inspectorConfig,terminal:inspectorTerminal})[state.tab](c)}</div><div class="inspector-footer"><span class="inspector-environment"><span>${esc(envTarget(reviewEnvironment)?.name||'本机')}</span><span>${isOffline()?'数据已陈旧':envTarget(reviewEnvironment)?.provider==='docker'?'Docker Engine':'Apple container'}</span></span><button data-copy="${esc(c.id)}">复制容器 ID ${icon('Copy')}</button></div></aside>`;
}
function inspectorOverview(c) {
  return `<h3>资源用量 <span class="muted" style="font-weight:400;float:right;font-size:10px">${isOffline()?'上次同步的样本':'最近 60 秒（示例）'}</span></h3><div class="metric-pair"><div class="metric-mini"><label>CPU（单核 100%）</label><strong>${c.status==='running'?c.cpu.toFixed(1):'—'} <small>${c.status==='running'?'%':''}</small></strong>${c.status==='running'?spark():'<div class="log-note">无实时样本</div>'}</div><div class="metric-mini"><label>内存 / 配置上限</label><strong>${c.status==='running'?c.memory:'—'} <small>${c.status==='running'?'MiB ':''}/ ${c.memoryLimit||512} MiB</small></strong>${c.status==='running'?spark(1):'<div class="log-note">无实时样本</div>'}</div></div><hr class="section-divider"><h3>连接</h3><div class="detail-row"><span>端口映射</span><div class="port-detail-actions"><button class="detail-link" data-edit-ports="${esc(c.id)}">${portMappings(c).length>1?`${portMappings(c).length} 条映射`:portMappings(c).length?`${portMappings(c)[0].host_port} → ${portMappings(c)[0].container_port}`:'添加映射'}</button>${portMappings(c).length?`<button class="icon-button" aria-label="复制端口映射" data-copy="${esc(portMappings(c).map(p=>`${p.host}:${p.host_port} → ${p.container_port}/${p.protocol}`).join('\n'))}">${icon('Copy')}</button>`:''}</div></div><div class="detail-row"><span>网络</span><span class="mono">default</span></div><div class="detail-row"><span>IPv4</span><span class="mono">${c.status==='running'?'192.168.64.'+(containers.indexOf(c)+2):'—'}</span></div><div class="detail-row"><span>健康检查</span><span>${esc(c.health||'未配置')}</span></div><hr class="section-divider"><h3>存储挂载</h3>${c.volume?`<div class="mount-line">${icon('Database')}<div>${esc(c.volume)}<small>→ ${esc(volumes.find(v=>v.name===c.volume)?.mount||'/data')}（读写）</small></div></div>`:'<div class="muted" style="font-size:11px">没有持久卷挂载</div>'}<hr class="section-divider"><h3 style="margin-bottom:7px">项目与平台</h3><span class="tag">${esc(c.project)}</span><span class="tag">linux / arm64</span>`;
}
function logText(c) {
  return `10:42:01 [info] Starting ${c.id}\n10:42:01 [info] Loading configuration\n10:42:02 [info] Database connection ready\n10:42:02 [info] Cache connection ready\n10:42:02 [info] Server is listening\n10:42:03 [info] GET /health 200 2ms\n10:42:18 [info] GET /api/projects 200 12ms\n10:42:25 [info] GET /api/containers 200 8ms\n10:43:02 [warn] Slow request: 284ms\n10:43:05 [info] GET /health 200 1ms\n10:43:18 [info] GET /api/projects 200 9ms\n10:43:22 [info] Request completed`;
}
function inspectorLogs(c) {
  return `<div class="log-controls"><span class="status-pill ${state.logFollow?'running':''}"><i class="status-dot ${state.logFollow?'online':''}"></i>${state.logFollow?'跟随日志':'已暂停跟随'}</span><div><button class="icon-button" id="toggle-log" aria-label="${state.logFollow?'暂停':'继续'}跟随日志">${icon(state.logFollow?'Pause':'Play')}</button><button class="icon-button" id="export-log" aria-label="导出示例日志">${icon('Download')}</button></div></div><div class="log-area" aria-label="示例容器日志">${esc(logText(c)).replace(/(\d{2}:\d{2}:\d{2})/g,'<span class="log-time">$1</span>').replace(/\[info\]/g,'<span class="log-ok">[info]</span>').replace(/\[warn\]/g,'<span class="log-warn">[warn]</span>')}</div><p class="log-note">示例日志 · 仅展示交互，不连接运行时。</p>`;
}
function inspectorConfig(c) {
  const rows = (items)=>items.map(([k,v])=>`<div class="config-row"><span>${k}</span><strong>${esc(v)}</strong></div>`).join('');
  return `<div class="config-toolbar"><button class="icon-button" onclick="refresh()" aria-label="刷新配置">${icon('RefreshCw')}</button><button class="button small" data-copy="${esc(JSON.stringify({name:c.id,platform:'linux/arm64',environmentNames:['PATH'],labelNames:['project']}))}">${icon('Copy')}复制 JSON</button></div><h3>基本信息</h3>${rows([['容器',c.id],['平台','linux / arm64'],['只读根文件系统','否'],['用户','未公开'],['工作目录','未公开']])}<h4 class="config-section">环境与标签</h4>${rows([['环境变量名','PATH'],['标签名','project']])}<h4 class="config-section">启动</h4>${rows([['启动参数','0 项']])}<p class="log-note">环境变量与标签仅展示名称，参数仅展示数量。</p>`;
}
function inspectorTerminal(c) {
  if (c.status !== 'running' || isOffline()) return `<div class="empty-state" style="padding:40px 8px">${icon('Terminal')}<h2 style="font-size:14px">暂时无法连接终端</h2><p>${isOffline()?'恢复运行时连接后再试。':'启动容器后可以建立交互会话。'}</p></div>`;
  return `<div class="terminal-note">模拟终端 · 不执行任何命令</div><div class="log-area" style="height:280px"><span class="log-ok">Connected to ${esc(c.id)}</span>\nAlpine Linux · linux/arm64\n\n/app $ pwd\n/app\n\n/app $ ls\nnode_modules  package.json\nsrc           data\n<div id="terminal-response"></div><form id="terminal-form" class="terminal-input"><span>/app $</span><input aria-label="模拟终端输入" placeholder="输入 help 体验" autocomplete="off" spellcheck="false"></form></div><p class="log-note">真实应用将使用原生 PTY，并支持 resize、Ctrl-C 与复制粘贴。</p>`;
}
function renderOverview() {
  return `${summary()}<div class="overview-grid"><section class="section-panel"><h2>容器资源趋势 <span class="muted" style="float:right;font-size:10px">示例 · 最近 1 小时</span></h2><div class="panel-body"><div class="overview-number">${containers.filter(c=>c.status==='running').reduce((s,c)=>s+c.cpu,0).toFixed(1)}% <small>容器 CPU（单核 100%）</small></div><svg class="large-chart" viewBox="0 0 600 156" preserveAspectRatio="none" role="img" aria-label="示例 CPU 趋势"><path d="M0 26H600M0 76H600M0 126H600" stroke="var(--border)" stroke-width="1" fill="none"/><path d="M0 126L25 120L50 125L75 98L100 104L125 100L150 115L175 76L200 82L225 68L250 90L275 70L300 84L325 45L350 61L375 54L400 77L425 58L450 69L475 36L500 47L525 39L550 55L575 31L600 42" fill="none" stroke="currentColor" stroke-width="2"/></svg><div class="chart-labels"><span>09:45</span><span>10:00</span><span>10:15</span><span>10:30</span><span>10:45</span></div></div></section><section class="section-panel"><h2>需要留意</h2><div class="panel-body"><div class="form-hint" style="background:var(--warn-soft);color:var(--warn)">${icon('CircleAlert')}<div><strong>mailpit 启动失败</strong><div style="margin-top:5px">端口 8025 已被占用，调整端口后重新启动。</div><button class="link-button" style="margin-top:12px" data-show="mailpit">查看容器 →</button></div></div><div class="detail-row" style="margin-top:23px"><span>运行时</span>${statusPill(isOffline()?'stopped':'running')}</div><div class="detail-row"><span>本地镜像</span><span>${images.length} 个</span></div><div class="detail-row"><span>存储卷</span><span>${volumes.length} 个</span></div><div class="detail-row"><span>网络</span><span>${networks.length} 个</span></div></div></section></div>${renderRecent()}<p class="scene-note">以上数据用于界面评审，不代表当前机器的实际负载。</p>`;
}
function resourceToolbar(placeholder) { return `<div class="list-toolbar"><label class="search-field" style="max-width:300px">${icon('Search')}<input id="generic-search" aria-label="筛选资源" placeholder="${placeholder}"></label><div class="toolbar-end"><span class="muted" style="font-size:11px">本机 · Apple container</span><button class="icon-button refresh-button" aria-label="刷新资源">${icon('RefreshCw')}</button></div></div>`; }
function renderVolumes() {
  return `${resourceToolbar('搜索存储卷…')}<div class="table-wrap resource-table-wrap"><table class="resource-table" aria-label="存储卷列表"><thead><tr><th style="padding-left:18px">名称</th><th>已用空间</th><th>使用者</th><th>容器路径</th><th style="text-align:right">操作</th></tr></thead><tbody>${volumes.map(v=>`<tr><td style="padding-left:18px"><div class="table-name-inline"><span class="image-symbol">${icon('Database')}</span><strong style="font-weight:550">${esc(v.name)}</strong></div></td><td class="numeric">${esc(v.size)}</td><td>${v.used?`<button class="link-button" data-show="${esc(v.used)}">${esc(v.used)}</button>`:'<span class="state-tag muted">未使用</span>'}</td><td class="mono muted" style="font-size:11px">${esc(v.mount)}</td><td style="text-align:right"><button class="icon-button" data-volume="${esc(v.name)}" aria-label="${esc(v.name)} 详情">${icon('Ellipsis')}</button></td></tr>`).join('')}</tbody></table></div><p class="scene-note">删除容器默认保留存储卷。删除卷前，应用会重新检查引用关系。</p>`;
}
function renderNetworks() {
  return `${resourceToolbar('搜索网络…')}<div class="table-wrap resource-table-wrap"><table class="resource-table" aria-label="网络列表"><thead><tr><th style="padding-left:18px">名称</th><th>子网</th><th>连接的容器</th><th>状态</th><th style="text-align:right">操作</th></tr></thead><tbody>${networks.map(n=>`<tr><td style="padding-left:18px"><div class="table-name-inline"><span class="image-symbol">${icon('Network')}</span><strong style="font-weight:550">${esc(n.name)}</strong></div></td><td class="mono" style="font-size:11px">${esc(n.subnet)}</td><td>${n.count} 个</td><td><span class="status-pill running"><i class="status-dot online"></i>可用</span></td><td style="text-align:right"><button class="button small" data-network="${esc(n.name)}">查看详情</button></td></tr>`).join('')}</tbody></table></div><p class="scene-note">网络和 DNS 功能随系统与运行时能力提供。动态修改连接需要额外能力探测。</p>`;
}
function renderTasks() {
  const visible = tasks.filter(t=>state.taskFilter==='all'||state.taskFilter===t.status);
  return `<div class="list-toolbar"><span class="muted" style="font-size:12px">${tasks.filter(t=>t.status==='busy').length} 项进行中 · ${tasks.filter(t=>t.status==='error').length} 项失败</span><select id="task-filter" class="filter-select" aria-label="任务状态"><option value="all">全部任务</option><option value="busy">进行中</option><option value="error">失败</option><option value="success">已完成</option></select></div><div class="task-list">${visible.length?visible.map(t=>`<article class="task-item ${t.status}">${icon(t.status==='error'?'CircleAlert':t.status==='busy'?'LoaderCircle':'CircleCheck',t.status==='busy'?'spin':'')}<div><strong>${esc(t.title)}</strong><p class="${t.status==='error'?'task-error':''}">${esc(t.detail)}</p><p>本机 / Apple container · 示例任务 #${t.id}</p>${t.status==='error'&&t.target?`<button class="button small" data-task-retry="${t.id}" ${isOffline()?'disabled':''}>${icon('RotateCw')}重试失败项</button>`:''}</div><small>${esc(t.time)}</small></article>`).join(''):'<div class="empty-state"><h2>没有这类任务</h2><p>切换筛选查看其他操作记录。</p></div>'}</div><p class="scene-note">操作记录与运行时事件分开保存。轮询发现的变化不代表完整事件历史。</p>`;
}
function navigate(page) {state.page=page;state.query='';state.containerPage=0;render();}
function showResource(id) {if(!resource(id)){toast('示例资源已不存在');return;}state.page='containers';state.selected=id;state.tab='overview';state.query='';state.filter='all';state.containerPage=0;render();}
function bindPage() {
  const search=$('#resource-search');
  search?.addEventListener('input',()=>{const pos=search.selectionStart;state.query=search.value;state.containerPage=0;render();$('#resource-search')?.focus();$('#resource-search')?.setSelectionRange(pos,pos);});
  const filter=$('#status-filter');if(filter){filter.value=state.filter;filter.onchange=()=>{state.filter=filter.value;state.containerPage=0;render();};}
  $$('.refresh-button').forEach(b=>b.onclick=refresh);
  $$('[data-nav]').forEach(b=>b.onclick=()=>navigate(b.dataset.nav));
  $$('[data-inspect]').forEach(b=>b.onclick=()=>{state.selected=b.dataset.inspect;state.tab='overview';render();});
  $$('[data-show]').forEach(b=>b.onclick=()=>showResource(b.dataset.show));
  $$('[data-life]').forEach(b=>b.onclick=()=>operate(b.dataset.life,[b.dataset.id]));
  $$('[data-more]').forEach(b=>b.onclick=()=>containerMenu(b.dataset.more));
  $$('[data-check]').forEach(b=>b.onchange=()=>{if(b.checked)state.checked.add(b.dataset.check);else state.checked.delete(b.dataset.check);render();});
  $('#select-all')?.addEventListener('change',e=>{containerPage().items.forEach(c=>e.target.checked?(state.checked.size<32&&state.checked.add(c.id)):state.checked.delete(c.id));render();});
  const all=$('#select-all');if(all)all.indeterminate=containerPage().items.some(c=>state.checked.has(c.id))&&!containerPage().items.every(c=>state.checked.has(c.id));
  $$('[data-batch]').forEach(b=>b.onclick=()=>b.dataset.batch==='delete'?deleteConfirm([...state.checked]):operate(b.dataset.batch,[...state.checked]));
  $('#clear-selection')?.addEventListener('click',()=>{state.checked.clear();render();});
  $('#empty-action')?.addEventListener('click',()=>{if(containers.length){state.query='';state.filter='all';state.containerPage=0;render();}else createDialog();});
  $('#close-inspector')?.addEventListener('click',collapseInspector);
  $$('[data-tab]').forEach(b=>{b.onclick=()=>{state.tab=b.dataset.tab;render();$(`#tab-${state.tab}`)?.focus();};b.onkeydown=e=>{if(['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();const tabs=['overview','logs','config','terminal'];state.tab=tabs[(tabs.indexOf(state.tab)+(e.key==='ArrowRight'?1:3))%4];render();$(`#tab-${state.tab}`).focus();}};});
  $$('[data-copy]').forEach(b=>b.onclick=()=>copyText(b.dataset.copy));
  $('#toggle-log')?.addEventListener('click',()=>{state.logFollow=!state.logFollow;render();});
  $('#export-log')?.addEventListener('click',()=>download(`${state.selected}-demo.log`,logText(resource(state.selected))));
  $('#duplicate-button')?.addEventListener('click',()=>createDialog(resource(state.selected).image,resource(state.selected)));
  $('#terminal-form')?.addEventListener('submit',e=>{e.preventDefault();const input=$('input',e.currentTarget), value=input.value.trim();$('#terminal-response').textContent = value==='help'?'示例会话：可输入 pwd 或 ls。\n':value==='pwd'?'/app\n':value==='ls'?'node_modules  package.json  src  data\n':'此设计稿不执行命令。输入 help 查看示例。\n';input.value='';});
  $('#generic-search')?.addEventListener('input',e=>{const q=e.target.value.toLowerCase();$$('tbody tr').forEach(r=>r.hidden=!r.textContent.toLowerCase().includes(q));});
  $$('[data-run-image]').forEach(b=>b.onclick=()=>{const im=images[+b.dataset.runImage];createDialog(`${im.name}:${im.tag}`);});
  $$('[data-image]').forEach(b=>b.onclick=()=>imageDialog(+b.dataset.image));
  $$('[data-volume]').forEach(b=>b.onclick=()=>volumeDialog(b.dataset.volume));
  $$('[data-network]').forEach(b=>b.onclick=()=>networkDialog(b.dataset.network));
  $('#runtime-toggle')?.addEventListener('click',()=>{if(isOffline()){setScenario('normal');toast('示例连接已恢复');}else runtimeStopDialog();});
  const taskFilter=$('#task-filter');if(taskFilter){taskFilter.value=state.taskFilter;taskFilter.onchange=()=>{state.taskFilter=taskFilter.value;render();};}
  $$('[data-task-retry]').forEach(b=>b.onclick=()=>{const t=tasks.find(t=>t.id===+b.dataset.taskRetry);operate(t.action||'start',[t.target],true);});
}
function operate(action,ids,retry=false) {
  if(isOffline()){toast('运行时离线，请先恢复连接');return;}
  const targets=ids.map(resource).filter(Boolean).filter(c=>!['starting','stopping','restarting'].includes(c.status));
  if(!targets.length){toast('没有可操作的目标');return;}
  const generation=state.generation;
  const label={start:'启动',stop:'停止',restart:'重启'}[action];
  const pending=targets.map(c=>{const t={id:taskId++,title:`${label} ${c.id}`,target:c.id,action,status:'busy',detail:'等待运行时确认…',time:'刚刚'};tasks.unshift(t);c.status=action==='start'?'starting':action==='stop'?'stopping':'restarting';return {c,t};});
  render();
  setTimeout(()=>{
    if(generation!==state.generation)return;
    let failed=0;
    for(const {c,t} of pending){
      const failure=state.scenario==='partial'&&c.id==='redis'&&!retry;
      if(failure){c.status='running';t.status='error';t.detail='操作超时，查询结果显示容器仍在运行。可重试此项。';failed++;}
      else {c.status=action==='stop'?'stopped':'running';c.uptime=c.status==='running'?'刚刚启动':'—';if(c.status==='running'&&!c.memory){c.memory=64;c.cpu=.2;}t.status='success';t.detail=action==='stop'?'容器已停止，存储卷已保留':'容器已运行，状态已确认';}
    }
    state.checked.clear();render();toast(failed?`${targets.length-failed} 项成功，${failed} 项失败 · 在任务中心重试失败项`:`${targets.length===1?targets[0].id:targets.length+' 个容器'}${action==='stop'?'已停止':'已启动'} · 示例操作`,!!failed);
  },650);
}
function toast(message,error=false) {clearTimeout(toastTimer);$('#toast').innerHTML=icon(error?'CircleAlert':'CircleCheck')+`<span>${esc(message)}</span>`;$('#toast').hidden=false;toastTimer=setTimeout(()=>$('#toast').hidden=true,4200);}
function refresh() {toast(isOffline()?'运行时仍离线，保留最后一次数据':'示例资源已刷新');}
async function copyText(value) {try{await navigator.clipboard.writeText(value);toast('已复制 '+value);}catch{openDialog('复制内容',`<p class="dialog-explainer">可选择并复制以下内容。</p><pre class="mono">${esc(value)}</pre>`);}}
function download(name,text) {const blob=new Blob([text],{type:'text/plain;charset=utf-8'});const url=URL.createObjectURL(blob);const link=document.createElement('a');link.href=url;link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);toast('示例日志已导出');}
function openDialog(title,body,footer='',sheet=false,subtitle='') {
  const modal=$('#modal');if(modal.open)modal.close();modalTrigger=document.activeElement;modal.className=sheet?'sheet':'';
  modal.innerHTML=`<div class="dialog-header"><div><h2 id="modal-title">${esc(title)}</h2>${subtitle?`<p>${esc(subtitle)}</p>`:''}</div><button class="icon-button close-modal" aria-label="关闭对话框">${icon('X')}</button></div><div class="dialog-content">${body}</div><div class="dialog-footer">${footer||'<button class="button close-modal">关闭</button>'}</div>`;
  $$('.close-modal',modal).forEach(b=>b.onclick=()=>modal.close());modal.showModal();return modal;
}
$('#modal').addEventListener('close',()=>{if(modalTrigger?.isConnected)modalTrigger.focus();});
function createDialog(image='ghcr.io/acme/api:dev',duplicate=null) {
  if(isOffline()){toast('运行时离线，请先恢复连接');return;}
  const body=`<div id="create-error" class="form-error" role="alert" hidden></div><div class="form-section"><h3><span class="step-number">1</span>镜像与名称</h3><label class="field"><span>容器镜像</span><input name="image" id="create-image" value="${esc(image)}" required placeholder="例如 nginx:alpine"><small>可以选择本地镜像，或输入完整的仓库地址。</small></label><label class="field"><span>容器名称</span><input name="name" id="create-name" value="${duplicate?esc(duplicate.id+'-copy'):''}" required placeholder="例如 api-preview" maxlength="63"><small>使用字母、数字、连字符、下划线或点。</small></label></div><div class="form-section"><h3><span class="step-number">2</span>资源与连接</h3><div class="field-row"><label class="field"><span>CPU 核数</span><input name="cpus" type="number" min="1" max="16" step="1" value="2" required></label><label class="field"><span>内存上限（MB）</span><input name="memory" type="number" min="64" max="32768" step="64" value="512" required></label></div><div class="field-row" style="margin-top:10px"><label class="field"><span>主机端口（可选）</span><input name="port" type="number" min="1" max="65535" placeholder="3001"></label><label class="field"><span>容器端口</span><input name="target" type="number" min="1" max="65535" placeholder="3000"></label></div><p class="log-note">端口默认绑定 127.0.0.1，仅允许本机访问。</p></div><details class="form-section"><summary>环境变量与高级设置</summary><label class="field"><span>环境变量（每行 KEY=VALUE）</span><textarea name="env" rows="3" placeholder="NODE_ENV=development"></textarea><small>设计稿请勿输入真实凭据。</small></label><label class="field"><span>项目分组</span><input name="project" value="playground"></label><p class="log-note">网络、挂载与更多运行参数在正式产品完整表单中提供。</p></details><div class="form-hint">${icon('Info')}<div>这次创建只会添加一条示例记录，不会下载镜像或在本机运行容器。</div></div>`;
  const modal=openDialog(duplicate?'复制容器':'新建容器',body,`<span>本机 · Apple container</span><button type="button" class="button close-modal">取消</button><button type="submit" form="create-form" class="button primary">${icon('Play')}创建并启动</button>`,true,'为应用创建一个独立的运行环境。');
  const content=$('.dialog-content',modal);const form=document.createElement('form');form.id='create-form';content.replaceWith(form);form.appendChild(content);
  // Keep the footer outside the form; the submit button uses the form attribute.
  form.onsubmit=e=>{e.preventDefault();const data=new FormData(form);const name=String(data.get('name')).trim();const img=String(data.get('image')).trim();const port=data.get('port')?Number(data.get('port')):null;const target=data.get('target')?Number(data.get('target')):null;
    let error='';if(!name||!img)error='请输入容器名称和镜像。';else if(!/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,62}$/.test(name))error='请输入有效的容器名称。';else if(resource(name))error='名称已存在，请换一个容器名称。';else if(!!port!==!!target)error='请同时填写主机端口和容器端口。';else if(port&&containers.some(c=>c.port===port&&c.status==='running'))error=`主机端口 ${port} 已被示例容器占用。`;
    if(error){$('#create-error').textContent=error;$('#create-error').hidden=false;return;}
    containers.unshift({id:name,image:img,status:'stopped',cpu:.3,memory:64,memoryLimit:Number(data.get('memory')),cpus:Number(data.get('cpus')),port,target,uptime:'—',volume:null,project:String(data.get('project')||'playground')});
    if(state.scenario==='empty'){state.scenario='normal';$('#scenario').value='normal';}
    modal.close();state.selected=name;state.tab='overview';state.page='containers';state.filter='all';state.query='';operate('start',[name]);
  };
  $('#create-name').focus();
}
function deleteConfirm(ids) {
  const targets=ids.map(resource).filter(Boolean);if(!targets.length)return;
  const modal=openDialog(`删除 ${targets.length===1?targets[0].id:targets.length+' 个容器'}？`,`<p class="dialog-explainer">删除会移除容器及其可写层。已运行的容器会先停止。关联的存储卷与镜像将保留。</p><ul class="impact-list">${targets.map(c=>`<li><strong>${esc(c.id)}</strong><small>本机 · ${esc(c.image)}</small></li>`).join('')}</ul><div class="form-hint">${icon('Database')}本次设计演示仅删除示例记录。</div>`,`<button class="button close-modal">取消</button><button id="confirm-delete" class="button danger" ${isOffline()?'disabled':''}>${icon('Trash2')}删除容器</button>`);
  $('#confirm-delete').onclick=()=>{targets.forEach(c=>{containers=containers.filter(x=>x.id!==c.id);state.checked.delete(c.id);tasks.unshift({id:taskId++,title:`删除 ${c.id}`,status:'success',detail:'示例容器已删除，持久卷和镜像保留',time:'刚刚'});});if(!resource(state.selected))state.selected=containers[0]?.id||null;modal.close();render();toast(`${targets.length} 个示例容器已删除，存储卷已保留`);};
}
function containerMenu(id) {
  const c=resource(id);const modal=openDialog(c.id,`<div class="command-list"><button id="menu-logs">${icon('ScrollText')}查看日志</button><button id="menu-terminal">${icon('Terminal')}打开终端</button><button id="menu-restart" ${isOffline()||c.status!=='running'?'disabled':''}>${icon('RotateCw')}重启容器</button><button id="menu-copy" ${isOffline()?'disabled':''}>${icon('CopyPlus')}复制配置并新建</button><button id="menu-delete" style="color:var(--danger)" ${isOffline()?'disabled':''}>${icon('Trash2')}删除容器…</button></div>`);
  $('#menu-logs').onclick=()=>{modal.close();state.selected=id;state.tab='logs';render();};$('#menu-terminal').onclick=()=>{modal.close();state.selected=id;state.tab='terminal';render();};$('#menu-restart').onclick=()=>{modal.close();operate('restart',[id]);};$('#menu-copy').onclick=()=>createDialog(c.image,c);$('#menu-delete').onclick=()=>deleteConfirm([id]);
}
function pullDialog() {
  if(isOffline()){toast('运行时离线，请先恢复连接');return;}
  const modal=openDialog('拉取镜像',`<label class="field"><span>镜像引用</span><input id="pull-reference" value="nginx:alpine" placeholder="registry/repository:tag"></label><label class="field"><span>平台</span><select><option>linux/arm64</option><option>linux/amd64</option></select></label><div class="form-hint">${icon('Info')}设计稿仅模拟拉取过程，不访问镜像仓库。</div>`,`<button class="button close-modal">取消</button><button class="button primary" id="confirm-pull">${icon('Download')}拉取镜像</button>`);
  $('#confirm-pull').onclick=()=>{const ref=$('#pull-reference').value.trim();if(!ref){$('#pull-reference').focus();return;}const t={id:taskId++,title:`拉取 ${ref}`,status:'busy',detail:'正在获取镜像信息…',time:'刚刚'};tasks.unshift(t);const generation=state.generation;modal.close();render();toast('镜像拉取已加入示例任务');setTimeout(()=>{if(generation!==state.generation)return;t.status='success';t.detail='示例镜像已就绪';const slash=ref.lastIndexOf('/'),colon=ref.lastIndexOf(':');const tagged=colon>slash;const name=tagged?ref.slice(0,colon):ref,tag=tagged?ref.slice(colon+1):'latest';if(!images.some(im=>im.name===name&&im.tag===tag))images.unshift({name,tag,size:'48 MB',used:0});render();toast('示例镜像已拉取完成');},700);};
}
function imageDialog(index) {
  const im=images[index];const modal=openDialog('镜像详情',`<div class="detail-row"><span>引用</span><span class="mono">${esc(im.name)}:${esc(im.tag)}</span></div><div class="detail-row"><span>平台</span><span>linux/arm64</span></div><div class="detail-row"><span>示例大小</span><span>${esc(im.size)}</span></div><div class="detail-row"><span>引用容器</span><span>${im.used} 个</span></div><p class="dialog-explainer" style="margin-top:18px">完整产品在这里提供 tag、push、save 与删除前引用检查。当前可体验从镜像新建容器。</p>`,`<button class="button close-modal">关闭</button><button class="button primary" id="image-create" ${isOffline()?'disabled':''}>${icon('Play')}从此镜像运行</button>`);$('#image-create').onclick=()=>{modal.close();createDialog(`${im.name}:${im.tag}`);};
}
function volumeDialog(name) {
  const v=volumes.find(v=>v.name===name);openDialog('存储卷 · '+name,`<div class="detail-row"><span>已用空间</span><span>${v.size}（示例）</span></div><div class="detail-row"><span>引用者</span><span>${esc(v.used||'无')}</span></div><div class="detail-row"><span>挂载路径</span><span class="mono">${esc(v.mount)}</span></div><div class="form-hint" style="margin-top:20px">${icon('ShieldCheck')}<div>${v.used?'此卷正在使用。真实产品会重新检查引用并禁止误删。':'此卷没有示例引用。正式删除需预览数据影响和重新核验。'}<br>备份与恢复流程将在完整管理阶段细化。</div></div>`);
}
function networkDialog(name) {
  const n=networks.find(n=>n.name===name);openDialog('网络 · '+name,`<div class="detail-row"><span>IPv4 子网</span><span class="mono">${esc(n.subnet)}</span></div><div class="detail-row"><span>连接数</span><span>${n.count} 个容器（示例）</span></div><div class="form-hint" style="margin-top:20px">${icon('Network')}网络详情展示关联资源。动态连接、断开或修改参数按运行时能力提供。</div>`);
}
function createResourceDialog(kind) {
  if(isOffline())return;
  const isVolume=kind==='volumes',label=isVolume?'存储卷':'网络';
  const modal=openDialog(`新建${label}`,`<div id="resource-error" class="form-error" hidden role="alert"></div><label class="field"><span>${label}名称</span><input id="new-resource-name" placeholder="${isVolume?'project-data':'project-network'}"></label>${!isVolume?'<label class="field"><span>IPv4 子网</span><input id="new-network-subnet" value="192.168.66.0/24"></label>':''}<p class="dialog-explainer">提交只添加示例资源，不会修改本机。</p>`,`<button class="button close-modal">取消</button><button class="button primary" id="confirm-resource">创建${label}</button>`);
  $('#confirm-resource').onclick=()=>{const name=$('#new-resource-name').value.trim(),list=isVolume?volumes:networks;let error='';if(!/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,62}$/.test(name))error='请输入有效的资源名称。';else if(list.some(x=>x.name===name))error='该名称已存在。';if(error){$('#resource-error').textContent=error;$('#resource-error').hidden=false;return;}if(isVolume)volumes.push({name,size:'0 MB',used:null,mount:'—'});else networks.push({name,subnet:$('#new-network-subnet').value.trim(),count:0});tasks.unshift({id:taskId++,title:`创建${label} ${name}`,status:'success',detail:'示例资源已创建',time:'刚刚'});modal.close();render();toast(`示例${label}已创建`);};
}
function runtimeStopDialog() {const modal=openDialog('停止运行时服务？',`<p class="dialog-explainer">这个动作会影响该环境的全部运行中容器。设计稿将切换至离线场景，真实机器不会发生变化。</p><ul class="impact-list"><li>本机 · Apple container<small>${containers.filter(c=>c.status==='running').length} 个运行中容器</small></li></ul>`,`<button class="button close-modal">取消</button><button class="button danger" id="confirm-runtime-stop">停止服务（模拟）</button>`);$('#confirm-runtime-stop').onclick=()=>{modal.close();setScenario('offline');toast('已切换为运行时离线场景');};}
function settingsDialog() {const modal=openDialog('设置与外观',`<div class="detail-row"><span>界面主题</span><button class="button small" id="dialog-theme">${document.documentElement.dataset.theme==='dark'?'切换浅色':'切换深色'}</button></div><div class="detail-row"><span>默认首页</span><span>容器</span></div><div class="detail-row"><span>退出行为</span><span>保留运行中的容器</span></div><hr class="section-divider"><p class="dialog-explainer">桌面实现将加入系统主题、菜单栏、语言、CLI 路径、日志缓存和诊断设置。</p>`);$('#dialog-theme').onclick=()=>{toggleTheme();modal.close();};}
function commandDialog() {
  const commands=[['容器','Container',()=>navigate('containers'),''],['镜像','Layers',()=>navigate('images'),''],['任务中心','ListChecks',()=>navigate('tasks'),''],['新建容器','Plus',createDialog,'⌘ N'],['拉取镜像','Download',pullDialog,''],['切换主题','Moon',toggleTheme,''],['刷新资源','RefreshCw',refresh,'⌘ R']];
  const modal=openDialog('搜索与命令',`<input id="command-search" class="command-input" placeholder="搜索页面或操作…" aria-label="搜索命令"><div class="command-list" id="command-list"></div>`,'<span>↑ ↓ 选择 · Enter 执行 · Esc 关闭</span><button class="button close-modal">关闭</button>');
  const draw=()=>{$('#command-list').innerHTML=commands.map((c,i)=>({c,i})).filter(({c})=>c[0].includes($('#command-search').value)).map(({c,i})=>`<button data-command="${i}">${icon(c[1])}${c[0]}<kbd>${c[3]}</kbd></button>`).join('');$$('[data-command]').forEach(b=>b.onclick=()=>{modal.close();commands[+b.dataset.command][2]();});};
  $('#command-search').oninput=draw;$('#command-search').onkeydown=e=>{if(e.key==='ArrowDown'){e.preventDefault();$('#command-list button')?.focus();}if(e.key==='Enter'){e.preventDefault();$('#command-list button')?.click();}};modal.onkeydown=e=>{if(e.target.matches('[data-command]')&&['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();const options=$$('[data-command]');options[(options.indexOf(e.target)+(e.key==='ArrowDown'?1:options.length-1))%options.length]?.focus();}};draw();$('#command-search').focus();
}
function toggleTheme() {const dark=document.documentElement.dataset.theme!=='dark';document.documentElement.dataset.theme=dark?'dark':'light';$('#theme-toggle').innerHTML=icon(dark?'Sun':'Moon')+`<span id="theme-label">${dark?'浅色':'深色'}</span>`;$('#theme-toggle').setAttribute('aria-label',dark?'切换浅色主题':'切换深色主题');}
function setScenario(scenario) {
  state.generation++;tasks=tasks.filter(t=>t.status!=='busy');
  state.containerPage=0;
  reviewServicePhase='idle';
  state.scenario=scenario;$('#scenario').value=scenario;state.checked.clear();state.query='';state.filter='all';
  containers=scenario==='empty'?[]:structuredClone(seedContainers);state.selected=containers[0]?.id||null;state.page='containers';state.tab='overview';render();
  if(scenario==='partial')toast('选择 web-api 和 redis 批量停止，redis 将模拟超时');
}
$$('#navigation [data-page]').forEach(b=>{b.setAttribute('aria-label',pageInfo[b.dataset.page][0]);b.onclick=()=>navigate(b.dataset.page);});
$('#primary-action').onclick=()=>{if(['containers','overview'].includes(state.page))createDialog();else if(state.page==='images')createDialog();else if(['volumes','networks'].includes(state.page))createResourceDialog(state.page);else if(state.page==='runtime')addEnvironmentDialog();else refresh();};
$('#secondary-action').onclick=()=>{if(state.page==='containers')pullDialog();else if(state.page==='images')pullDialog();else if(state.page==='volumes')openDialog('空间概况',`<p class="dialog-explainer">示例卷共使用约 3.7 GB。真实清理将先查询 system df、引用关系和可回收资源，再展示逐项预览。</p>`);else if(state.page==='networks')networkDialog(networks[0].name);else if(state.page==='tasks'){state.taskFilter='all';render();}else if(state.page==='runtime')environmentConnectionTest(inspectedEnvironment);else refresh();};
$('#scenario').onchange=e=>setScenario(e.target.value);
$('#theme-toggle').onclick=toggleTheme;
$('#width-toggle').onclick=()=>{const narrow=$('#app').classList.toggle('narrow');$('#width-toggle').setAttribute('aria-pressed',String(narrow));$('#width-toggle').textContent=narrow?'标准窗口':'窄窗口';};
$('#command-button').onclick=commandDialog;
$('#environment-button').onclick=environmentDialog;
$('#settings-button').onclick=settingsDialog;
$('#reconnect-button').onclick=()=>navigate('runtime');
document.addEventListener('keydown',e=>{if(e.isComposing)return;const command=e.metaKey||e.ctrlKey;if(!command)return;const key=e.key.toLowerCase();if($('#modal').open)return;if(e.target.closest('#terminal-form'))return;if(['k','n','r','f',','].includes(key)){e.preventDefault();if(key==='k')commandDialog();if(key==='n')createDialog();if(key==='r')refresh();if(key==='f')($('#resource-search')||$('#generic-search'))?.focus();if(key===',')settingsDialog();}});
const reviewParams = new URLSearchParams(location.search);
if(innerWidth < 1280 || reviewParams.has('list')) state.selected=null;
if(reviewParams.has('inspect')) state.selected='web-api';
if(['overview','logs','config'].includes(reviewParams.get('tab'))) state.tab=reviewParams.get('tab');
if(reviewParams.has('multi')) { selectedImages.add(0); selectedImages.add(2); }
if(reviewParams.get('theme')==='dark') toggleTheme();
if([5,10,20,50].includes(Number(reviewParams.get('pageSize')))) state.pageSize=Number(reviewParams.get('pageSize'));
state.containerPage=Math.max(0,(Number(reviewParams.get('containerPage'))||1)-1);
if(reviewParams.get('scenario')==='offline') { state.scenario='offline'; $('#scenario').value='offline'; }
if(reviewParams.get('service')==='unknown') { reviewServicePhase='unknown'; state.scenario='offline'; $('#scenario').value='offline'; }
if(pageInfo[reviewParams.get('page')]) state.page=reviewParams.get('page');
if(envTarget(reviewParams.get('environment'))) inspectedEnvironment=reviewParams.get('environment');
if(state.page==='stacks'){reviewEnvironment='docker';inspectedEnvironment='docker';syncEnvironmentChrome();}
render();

if(reviewParams.has('ports') && state.selected) portEditor(state.selected);
