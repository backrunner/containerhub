function renderEnvironmentRail() {
  const query=environmentQuery.trim().toLowerCase();
  const targets=reviewTargets.filter(t=>`${t.name} ${t.host} ${t.description}`.toLowerCase().includes(query));
  return `<aside class="env-rail" aria-label="环境列表"><label class="env-search">${icon('Search')}<input id="environment-search" placeholder="查找环境…" aria-label="查找环境" value="${esc(environmentQuery)}"></label><div class="env-list-scroll">${['本机','远程'].map(group=>{
    const rows=targets.filter(t=>t.group===group);if(!rows.length)return '';
    return `<div class="env-list-label">${group}<span>${rows.length}</span></div><div role="list" aria-label="${group}环境">${rows.map(t=>`<div class="env-list-row ${t.id===inspectedEnvironment?'selected':''}" role="listitem" data-environment-row="${t.id}"><button class="env-select" data-env-inspect="${t.id}" aria-pressed="${t.id===inspectedEnvironment}" aria-label="查看 ${esc(t.name)}"><span class="env-list-icon">${icon(t.transport==='local'?'Laptop':'Server')}<i class="env-connection-dot ${environmentIsOffline(t)?'offline':''}"></i></span><span class="env-list-text"><strong>${esc(t.name)}</strong><small>${t.needsCheck?'等待身份验证':environmentIsOffline(t)?'离线 · '+(t.lastSeen||'刚刚'):transportLabel(t)}</small></span>${t.id===reviewEnvironment?'<span class="env-current">当前</span>':''}</button><button class="icon-button env-row-menu" data-env-menu="${t.id}" aria-label="${esc(t.name)} 更多操作" aria-haspopup="menu">${icon('Ellipsis')}</button></div>`).join('')}</div>`;
  }).join('')}${!targets.length?'<div class="env-empty-search">没有匹配的环境<button class="link-button" id="clear-environment-search">清除搜索</button></div>':''}</div></aside>`;
}
function renderRuntime() {
  return `<div class="env-layout">${renderEnvironmentRail()}<div class="env-detail-scroll" id="environment-detail" aria-label="主机详情">${renderEnvironmentHost(inspectedTarget())}</div></div>`;
}
function renderEnvironmentHost(t) {
  if(t.provider==='docker')return renderDockerEnvironment(t);
  const offline=environmentIsOffline(t);
  return `<header class="host-heading"><div class="host-identity"><span class="host-glyph">${icon(t.transport==='local'?'Laptop':'Server')}</span><div><div class="host-title"><h2>${esc(t.name)}</h2><span class="env-status ${offline?'offline':''}"><i></i>${t.needsCheck?'待验证':offline?'离线':'已连接'}</span></div><p>${esc(t.host)}<span>·</span>${t.provider==='docker'?'Docker Engine':'Apple container 1.6.0'}</p></div></div><div class="host-actions">${t.id===reviewEnvironment?'<span class="host-current">当前环境</span>':`<button class="button" data-env-activate="${t.id}" ${offline?'disabled':''}>切换</button>`}<button class="icon-button bordered" data-env-edit="${t.id}" aria-label="编辑 ${esc(t.name)}">${icon('Settings2')}</button></div></header>
  ${t.description?`<p class="host-description">${esc(t.description)}</p>`:''}
  <dl class="host-facts"><div><dt>系统</dt><dd>${esc(t.os)} <span>/ ${t.arch}</span></dd></div><div><dt>处理器</dt><dd>${esc(t.processor)} <span>/ ${t.cores} 核</span></dd></div><div><dt>主机</dt><dd>${esc(t.hardware)}</dd></div><div><dt>${offline?'断开前运行时间':'运行时间'}</dt><dd>${t.uptime}</dd></div></dl>
  ${offline?`<div class="env-offline-note">${icon('WifiOff')}<div><strong>${t.needsCheck?'连接配置已更改，等待验证':'连接已中断'}</strong><p>保留${esc(t.lastSeen||'刚刚')}的最后采样，当前资源与健康状态未知。</p></div><button class="button" data-env-test="${t.id}">检查连接</button></div>`:''}
  ${renderEnvironmentMetrics(t)}${renderEnvironmentContainers(t)}${renderEnvironmentConnection(t)}
  <details class="env-capabilities"><summary><span>运行时能力${offline?' · 上次检测':''}</span><span>${t.transport==='local'?'21':'20'} / 22 项已支持</span>${icon('ChevronDown')}</summary><div class="capability-grid">${reviewCapabilities.map((n,i)=>`<div class="capability-item"><span>${n}</span><small class="${(i===0&&t.transport!=='local')||n==='Stack 管理'?'unavailable':''}">${n==='Stack 管理'?'未支持':i===0&&t.transport!=='local'?'仅限本机':'已支持'}</small></div>`).join('')}</div></details>`;
}
function renderEnvironmentContainers(t) {
  const c=t.containers,total=c.running+c.stopped+c.error,checked=c.healthy+c.unhealthy,offline=environmentIsOffline(t);
  return `<section class="env-section" aria-labelledby="env-containers-title"><div class="env-section-heading"><h3 id="env-containers-title">容器概览 <span>${total}</span></h3><button class="link-button" data-env-containers="all">查看容器 ${icon('ArrowUpRight')}</button></div><div class="env-container-overview ${offline?'stale':''}"><div class="env-counts">${[['running','运行中',c.running],['stopped','已停止',c.stopped],['error','状态未知',c.error]].map(([key,label,value])=>`<button data-env-containers="${key}" class="env-count ${key}"><span>${label}</span><strong class="tabular">${value}</strong></button>`).join('')}</div><div class="env-health"><div class="env-health-heading"><span>健康检查</span><strong>${offline?'状态待更新':`${checked} 个已检查`}</strong></div><div class="health-track" role="img" aria-label="${c.healthy} 个健康，${c.unhealthy} 个不健康，${c.unchecked} 个未配置检查">${[['healthy',c.healthy],['unhealthy',c.unhealthy],['unchecked',c.unchecked]].filter(([,n])=>n).map(([key,n])=>`<span class="${key}" style="flex:${n}"></span>`).join('')}</div><div class="health-legend">${[['healthy','健康',c.healthy],['unhealthy','不健康',c.unhealthy],['unchecked','未配置',c.unchecked]].map(([key,label,n])=>`<button data-env-containers="${key}" class="${key}"><i></i>${label}<span>${n}</span></button>`).join('')}</div></div></div>${c.unhealthy?`<div class="env-issue">${icon('CircleAlert')}<span>${offline?'断开前：':''}<strong>api-worker</strong> 健康检查连续 3 次失败</span><button class="link-button" data-env-containers="unhealthy">查看</button></div>`:''}</section>`;
}
function renderEnvironmentConnection(t) {
  const local=t.transport==='local',offline=environmentIsOffline(t);
  return `<section class="env-section connection-section" aria-labelledby="env-connection-title"><div class="env-section-heading"><h3 id="env-connection-title">${local?'本机连接':'连接信息'}</h3><div class="connection-actions">${!local?`<span class="connection-secure">${icon('ShieldCheck')}${offline?'绑定已保留':'端到端加密'}</span>`:''}<button class="icon-button bordered" data-env-test="${t.id}" aria-label="检查 ${esc(t.name)} 的连接">${icon('RefreshCw')}</button></div></div>
  <div class="connection-route ${offline?'stale':''}" aria-label="连接路径"><span>${icon('Laptop')}此设备</span><i></i>${t.transport==='relay'?`<span>${icon('Globe')}新加坡中继</span><i></i>`:''}<span>${icon(local?'Cpu':'Server')}${local?'Apple container':esc(t.host)}</span><small>${local?'本机 CLI':transportLabel(t)}</small></div>
  <dl class="connection-facts">${local?`<div class="wide"><dt>CLI 路径</dt><dd><code>${t.id==='demo'?'示例运行时':esc(t.cli||'/opt/homebrew/bin/container')}</code><button class="icon-button" data-env-edit="${t.id}" aria-label="编辑 CLI 路径">${icon('Settings2')}</button></dd></div><div><dt>服务状态</dt><dd>${offline?'不可连接':'可连接'}</dd></div><div><dt>采样频率</dt><dd>每 ${t.refresh} 秒</dd></div>`:`<div class="wide"><dt>${t.transport==='relay'?'中继地址':'目标地址'}</dt><dd><code>${esc(t.transport==='relay'?t.relay:t.endpoint)}</code><button class="icon-button" data-env-copy="${t.id}" aria-label="复制连接地址">${icon('Copy')}</button></dd></div>${t.transport==='relay'?`<div class="wide"><dt>目标地址</dt><dd><code>${esc(t.endpoint)}</code><span>${t.policy==='relay'?'已停用直连':'备用直连'}</span></dd></div>`:''}<div><dt>连接策略</dt><dd>${{auto:'优先直连，失败走中继',direct:'仅直连',relay:'仅中继'}[t.policy]}</dd></div><div><dt>请求耗时</dt><dd>${offline?'—':`${t.latency} ms`}<span>${offline?'连接中断':'含身份验证'}</span></dd></div><div><dt>最近响应</dt><dd data-heartbeat>${offline?t.lastSeen||'刚刚':'刚刚'}</dd></div><div><dt>Agent 版本</dt><dd>${esc(t.agent)}</dd></div><div><dt>绑定时间</dt><dd>${esc(t.paired)}</dd></div><div class="wide"><dt>Agent 身份</dt><dd><button class="identity-button" data-env-identity="${t.id}" aria-label="查看完整 Agent 指纹"><code>${t.fingerprint.slice(0,18)}…${t.fingerprint.slice(-12)}</code>${icon('ShieldCheck')}</button><span>${offline?'已固定身份':'身份已验证'}</span></dd></div>`}</dl>${local&&offline?renderRuntimeService():''}</section>`;
}
function renderEnvironmentDetail() {
  const detail=document.querySelector('#environment-detail');if(!detail)return;
  const scroll=detail.scrollTop;detail.innerHTML=renderEnvironmentHost(inspectedTarget());detail.scrollTop=scroll;bindEnvironmentDetail();
}
function inspectEnvironment(id) {
  if(!envTarget(id))return;inspectedEnvironment=id;
  closeEnvironmentContextMenu();
  const rail=document.querySelector('.env-rail');if(rail)rail.outerHTML=renderEnvironmentRail();
  renderEnvironmentDetail();document.querySelector('#environment-detail')?.scrollTo({top:0});bindEnvironmentRail();document.querySelector(`[data-env-inspect="${id}"]`)?.focus();
}
function syncEnvironmentChrome() {
  const t=envTarget(reviewEnvironment)||reviewTargets[0];
  document.querySelector('#environment-button strong').textContent=t.name;
  document.querySelector('.workspace-mark small').textContent=t.name;
  document.querySelector('.crumb-runtime').textContent=t.provider==='docker'?'Docker Engine':'Apple container';
  if(state.page==='stacks'&&t.provider!=='docker')state.page='containers';
}
function selectReviewEnvironment(id) {
  const t=envTarget(id);if(!t)return;
  if(environmentIsOffline(t)){toast('环境离线，请先检查连接。');return;}
  reviewEnvironment=id;inspectedEnvironment=id;syncEnvironmentChrome();
  document.querySelector('.environment-menu')?.remove();document.querySelector('#environment-button').setAttribute('aria-expanded','false');
  render();toast(`已切换到 ${t.name} · 设计示例`);
}
function environmentDialog() {
  const previous=document.querySelector('.environment-menu');if(previous){previous.remove();document.querySelector('#environment-button').setAttribute('aria-expanded','false');return;}
  const menu=document.createElement('div');menu.className='environment-menu';menu.setAttribute('role','menu');
  menu.innerHTML=reviewTargets.map(t=>`<button role="menuitem" data-environment="${t.id}" ${t.offline?'disabled':''}>${icon(t.transport==='local'?'Laptop':'Server')}${esc(t.name)}<span>${t.id===reviewEnvironment?icon('Check'):''}</span></button>`).join('')+`<hr><button role="menuitem" id="manage-environments">${icon('Server')}环境管理…</button>`;
  document.querySelector('#app').append(menu);document.querySelector('#environment-button').setAttribute('aria-expanded','true');
  menu.querySelectorAll('[data-environment]').forEach(b=>b.onclick=()=>selectReviewEnvironment(b.dataset.environment));
  menu.querySelector('#manage-environments').onclick=()=>{menu.remove();document.querySelector('#environment-button').setAttribute('aria-expanded','false');navigate('runtime');};
}
function bindEnvironmentRail() {
  const search=document.querySelector('#environment-search');
  if(search)search.oninput=()=>{environmentQuery=search.value;const pos=search.selectionStart;document.querySelector('.env-rail').outerHTML=renderEnvironmentRail();bindEnvironmentRail();const input=document.querySelector('#environment-search');input.focus();input.setSelectionRange(pos,pos);};
  document.querySelector('#clear-environment-search')?.addEventListener('click',()=>{environmentQuery='';document.querySelector('.env-rail').outerHTML=renderEnvironmentRail();bindEnvironmentRail();document.querySelector('#environment-search').focus();});
  document.querySelectorAll('[data-env-inspect]').forEach(b=>b.onclick=()=>inspectEnvironment(b.dataset.envInspect));
  document.querySelectorAll('[data-environment-row]').forEach(row=>{
    row.oncontextmenu=e=>{e.preventDefault();openEnvironmentContextMenu(row.dataset.environmentRow,{x:e.clientX,y:e.clientY},row.querySelector('.env-select'));};
    row.onkeydown=e=>{if(e.key==='ContextMenu'||(e.shiftKey&&e.key==='F10')){e.preventDefault();const r=row.getBoundingClientRect();openEnvironmentContextMenu(row.dataset.environmentRow,{x:r.left+20,y:r.bottom},row.querySelector('.env-select'));}};
  });
  document.querySelectorAll('[data-env-menu]').forEach(b=>b.onclick=()=>{const r=b.getBoundingClientRect();openEnvironmentContextMenu(b.dataset.envMenu,{x:r.right,y:r.bottom+4},b);});
}
function bindEnvironmentDetail() {
  document.querySelectorAll('[data-env-activate]').forEach(b=>b.onclick=()=>selectReviewEnvironment(b.dataset.envActivate));
  document.querySelectorAll('[data-env-edit]').forEach(b=>b.onclick=()=>environmentEditDialog(b.dataset.envEdit));
  document.querySelectorAll('[data-env-test]').forEach(b=>b.onclick=()=>environmentConnectionTest(b.dataset.envTest));
  document.querySelectorAll('[data-env-copy]').forEach(b=>b.onclick=()=>{const t=envTarget(b.dataset.envCopy);copyText(t.transport==='relay'?t.relay:t.endpoint);});
  document.querySelectorAll('[data-env-identity]').forEach(b=>b.onclick=()=>environmentIdentityDialog(b.dataset.envIdentity));
  document.querySelectorAll('[data-env-containers]').forEach(b=>b.onclick=()=>environmentContainersDialog(inspectedEnvironment,b.dataset.envContainers));
  bindEnvironmentCharts();bindRuntimeService();
}
function bindEnvironment() {
  bindEnvironmentRail();bindEnvironmentDetail();
  document.querySelector('#sort-field')?.addEventListener('click',()=>{state.containerPage=0;containers.sort((a,b)=>a.status.localeCompare(b.status)||a.id.localeCompare(b.id));render();});
  document.querySelector('#sort-direction')?.addEventListener('click',()=>{state.containerPage=0;containers.reverse();render();});
}
document.addEventListener('click',e=>{if(!e.target.closest('.environment-menu,#environment-button')){document.querySelector('.environment-menu')?.remove();document.querySelector('#environment-button')?.setAttribute('aria-expanded','false');}});
document.addEventListener('keydown',e=>{if(e.key==='Escape'){document.querySelector('.environment-menu')?.remove();document.querySelector('#environment-button')?.setAttribute('aria-expanded','false');}});
