/* v8.1: isolated review data, never calls the host runtime. */
const stackFixtures = [
  {name:'shop',kind:'compose',services:[
    {name:'minio',image:'minio/minio:latest',state:'running',health:'不健康',ports:''},
    {name:'nginx-proxy',image:'nginx:alpine',state:'running',health:'未配置',ports:''},
    {name:'postgres',image:'postgres:17-alpine',state:'running',health:'健康',ports:''},
    {name:'redis',image:'redis:7-alpine',state:'running',health:'健康',ports:''},
    {name:'web-api',image:'ghcr.io/example/api:dev',state:'running',health:'健康',ports:'3000 → 3000/tcp'}]},
  {name:'tools',kind:'compose',services:[
    {name:'mailpit',image:'axllent/mailpit:latest',state:'stopped',health:'—',ports:''},
    {name:'sandbox',image:'alpine:3.22',state:'stopped',health:'—',ports:''},
    {name:'worker',image:'ghcr.io/example/api:dev',state:'stopped',health:'—',ports:''}]},
  ...swarmStackFixtures
];
const stackIdentity = s => s.kind+':'+s.name;
const stackKindLabel = s => s.kind==='swarm'?'Swarm':'Compose';
let stackSelection='compose:shop', stackQuery='', stackKind='', stackAttention=false;
const stackChecked=new Set(), stackExpanded=new Set(), stackContainerPages=new Map();
const stackToken=(s,c)=>stackIdentity(s)+':'+c.name;
const stackRunning=c=>c.tasks?c.tasks.filter(t=>t.state==='running'&&t.desired==='running').length:Number(c.state==='running');
function stackStatus(c) {
  if(c.tasks){
    if(c.update?.includes('paused'))return ['更新暂停','danger'];
    if(c.truncated)return ['采集不完整','warning'];
    if(c.mode==='Job')return ['一次性任务','muted'];
    if(c.mode==='Unknown')return ['状态未知','warning'];
    if(['updating','rollback_started'].includes(c.update))return ['更新中','updating'];
    const desired=c.replicas??c.tasks.filter(t=>t.desired==='running').length;
    if(!desired)return stackRunning(c)?['更新中','updating']:c.mode==='Replicated'?['已停止','muted']:['状态未知','warning'];
    return stackRunning(c)<desired?['调度中','warning']:stackRunning(c)>desired?['更新中','updating']:['运行中','success'];
  }
  return c.state==='running'?(c.health==='不健康'?['不健康','danger']:['运行中','success']):['已停止','muted'];
}
const stackNeedsAttention=c=>['danger','warning'].includes(stackStatus(c)[1]);
const stackServiceMatches=c=>(c.name+' '+c.image).toLowerCase().includes(stackQuery.trim().toLowerCase());
const visibleStacks=()=>stackFixtures.filter(s=>(!stackKind||s.kind===stackKind)&&(s.name.toLowerCase().includes(stackQuery.trim().toLowerCase())||s.services.some(stackServiceMatches)));
const visibleStackServices=s=>s.services.filter(c=>(!stackAttention||stackNeedsAttention(c))&&(s.name.toLowerCase().includes(stackQuery.trim().toLowerCase())||stackServiceMatches(c)));
const selectedStack=()=>visibleStacks().find(s=>stackIdentity(s)===stackSelection)||visibleStacks()[0];
const stackIconButton=(glyph,title,attrs,disabled=false,danger=false)=>`<button class="icon-button bordered ${danger?'danger':''}" aria-label="${title}" title="${title}" ${attrs} ${disabled?'disabled':''}>${icon(glyph)}</button>`;
function stackActions(stack, scope) {
  const affected=scope||stack.services;
  return `<div class="stack-actions">${stack.kind==='swarm'?`<button class="button" data-stack-action="restart" ${affected.some(c=>!['Global','Replicated'].includes(c.mode))?'disabled':''}>${icon('RotateCw')}滚动重启</button>`:
    `<button class="button" data-stack-action="start" ${affected.every(c=>c.state!=='stopped')?'disabled':''}>${icon('Play')}启动</button><button class="button danger" data-stack-action="stop" ${affected.every(c=>c.state!=='running')?'disabled':''}>${icon('Square')}停止</button>${stackIconButton('RotateCw','重启','data-stack-action="restart"',affected.every(c=>c.state!=='running'))}`}
    ${stackIconButton('Ellipsis','更多操作','data-stack-menu="all"')}</div>`;
}
function renderStacks(){
  if(envTarget(reviewEnvironment)?.provider!=='docker')return '<div class="empty-state">切换到 Docker 环境查看 Stacks</div>';
  const items=visibleStacks(), stack=selectedStack();
  return `<div class="stack-page"><div class="stack-toolbar"><label class="search-field">${icon('Search')}<input id="stack-search" placeholder="搜索 Stack、服务或镜像…" value="${esc(stackQuery)}"></label><div class="stack-kind-tabs" aria-label="Stack 类型">${[['','全部'],['compose','Compose'],['swarm','Swarm']].map(([k,label])=>`<button data-stack-kind="${k}" aria-pressed="${stackKind===k}">${label}</button>`).join('')}</div></div>
    ${stack?`<div class="stack-columns"><aside class="stack-rail" aria-label="Stack 列表"><header><span>STACKS</span><span>${items.length}</span></header><div class="stack-rail-scroll">${items.map(s=>{
      const attention=s.services.filter(stackNeedsAttention).length,running=s.services.filter(c=>stackRunning(c)>0).length;
      return `<button data-stack-select="${stackIdentity(s)}" class="${s===stack?'selected':''}" aria-pressed="${s===stack}"><span class="stack-rail-heading">${icon('Layers')}<strong title="${esc(s.name)}">${esc(s.name)}</strong><span class="stack-kind-badge">${stackKindLabel(s)}</span></span><span class="stack-rail-summary"><span>${s.services.length} 个服务</span><span class="${attention?'warning':''}">${attention?attention+' 需关注':running+' 运行中'}</span></span></button>`;
    }).join('')}</div></aside>${renderStackDetail(stack)}</div>`:'<div class="stack-empty">'+icon('Layers')+'<span>没有匹配的 Stack</span></div>'}</div>`;
}
function renderStackDetail(stack){
  const selected=stack.services.filter(c=>stackChecked.has(stackToken(stack,c))),visible=visibleStackServices(stack),attention=stack.services.filter(stackNeedsAttention).length;
  return `<section class="stack-detail"><header><div class="stack-title"><h2>${esc(stack.name)}</h2><span class="stack-kind-badge">${stackKindLabel(stack)}</span></div>${stackActions(stack,selected.length?selected:null)}</header>
    <div class="stack-metrics"><div><span>服务</span><strong>${stack.services.length}</strong></div><div><span>${stack.kind==='swarm'?'运行副本':'运行容器'}</span><strong>${stack.services.some(c=>c.truncated)?'≥':''}${stack.services.reduce((n,c)=>n+stackRunning(c),0)}</strong></div><div><span>需关注</span><strong class="${attention?'warning':''}">${attention}</strong></div></div>
    <div class="stack-service-toolbar"><strong>${selected.length?'已选 '+selected.length+' 个服务':visible.length+' 个服务'}</strong>${selected.length?'<button class="stack-text-button" id="stack-clear">清除</button>':''}<button class="stack-text-button ${stackAttention?'active':''}" id="stack-attention" aria-pressed="${stackAttention}">仅需关注</button></div>
    <div class="stack-table-head stack-grid"><input type="checkbox" id="stack-select-all" aria-label="选择当前筛选的服务" ${visible.length&&visible.every(c=>stackChecked.has(stackToken(stack,c)))?'checked':''} ${!visible.length?'disabled':''}><span></span><span>服务 / 镜像</span><span>状态 / 实例</span><span class="stack-port-col">端口</span><span class="stack-action-heading">操作</span></div>
    <div class="stack-service-scroll">${visible.map(c=>renderStackService(stack,c)).join('')||'<div class="stack-empty">'+(stackAttention?'没有需要关注的服务':'没有匹配的服务')+'</div>'}</div></section>`;
}
function renderStackService(stack,c){
  const i=stack.services.indexOf(c),token=stackToken(stack,c),expanded=stackExpanded.has(token),checked=stackChecked.has(token),[status,tone]=stackStatus(c),running=stackRunning(c),swarm=stack.kind==='swarm';
  const count=swarm?(c.replicas===null?c.mode+'（'+running+' 副本）':running+' / '+c.replicas+' 副本'):running+' / 1 容器';
  return `<section class="stack-service" data-stack-service="${i}"><div class="stack-grid stack-service-row ${checked?'selected':''}"><input type="checkbox" aria-label="选择服务 ${esc(c.name)}" data-stack-check="${i}" ${checked?'checked':''}><button class="stack-disclosure" data-stack-expand="${i}" aria-label="${expanded?'收起':'展开'} ${esc(c.name)}" aria-expanded="${expanded}">${icon(expanded?'ChevronDown':'ChevronRight')}</button><button class="stack-service-name" data-stack-expand="${i}" aria-expanded="${expanded}"><strong>${esc(c.name)}</strong><small title="${esc(c.image)}">${esc(c.image)}</small></button><div class="stack-service-status"><span class="${tone}"><i class="stack-dot"></i>${status}</span><small>${c.truncated?'≥':''}${count}</small></div><span class="stack-port-col">${esc(c.ports||'—')}</span><div class="stack-row-actions">${swarm?stackIconButton('SlidersHorizontal','调整副本数',`data-swarm-scale="${i}"`,c.mode!=='Replicated'):stackIconButton(running?'Square':'Play',running?'停止':'启动',`data-stack-row-action="${running?'stop':'start'}" data-service-index="${i}"`,false,!!running)}${stackIconButton('RotateCw',swarm?'滚动重启此服务':'重启此服务',`data-stack-row-action="restart" data-service-index="${i}"`,swarm?!['Replicated','Global'].includes(c.mode):!running)}${stackIconButton('Ellipsis','服务操作',`data-stack-menu="${i}"`)}</div></div>${expanded?renderContainerOverview(stack,c,i) :''}</section>`;
}
function bindStacks(){
  if(state.page!=='stacks')return;
  const stack=selectedStack();if(!stack)return bindStackFilters();stackSelection=stackIdentity(stack);
  bindStackFilters();
  document.querySelectorAll('[data-stack-select]').forEach(b=>b.onclick=()=>{stackSelection=b.dataset.stackSelect;stackChecked.clear();stackAttention=false;render();});
  document.querySelector('#stack-attention').onclick=()=>{stackAttention=!stackAttention;render();};
  document.querySelector('#stack-clear')?.addEventListener('click',()=>{stackChecked.clear();render();});
  document.querySelectorAll('[data-stack-check]').forEach(b=>b.onchange=()=>{const token=stackToken(stack,stack.services[Number(b.dataset.stackCheck)]);b.checked?stackChecked.add(token):stackChecked.delete(token);render();});
  const all=document.querySelector('#stack-select-all'),visible=visibleStackServices(stack);all.indeterminate=visible.some(c=>stackChecked.has(stackToken(stack,c)))&&!visible.every(c=>stackChecked.has(stackToken(stack,c)));
  all.onchange=()=>{visible.forEach(c=>{const token=stackToken(stack,c);all.checked?stackChecked.add(token):stackChecked.delete(token);});render();};
  document.querySelectorAll('[data-stack-expand]').forEach(b=>b.onclick=()=>{const token=stackToken(stack,stack.services[Number(b.dataset.stackExpand)]);stackExpanded.has(token)?stackExpanded.delete(token):stackExpanded.add(token);render();});
  document.querySelectorAll('[data-stack-action]').forEach(b=>b.onclick=()=>{const checked=stack.services.filter(c=>stackChecked.has(stackToken(stack,c)));confirmStackAction(stack,b.dataset.stackAction,checked.length?checked:stack.services);});
  document.querySelectorAll('[data-stack-row-action]').forEach(b=>b.onclick=()=>confirmStackAction(stack,b.dataset.stackRowAction,[stack.services[Number(b.dataset.serviceIndex)]]));
  document.querySelectorAll('[data-stack-menu]').forEach(b=>b.onclick=()=>openStackMenu(stack,b.dataset.stackMenu,b));
  document.querySelectorAll('[data-stack-service]').forEach(row=>row.oncontextmenu=e=>{e.preventDefault();openStackMenu(stack,row.dataset.stackService,row,{x:e.clientX,y:e.clientY});});
  document.querySelectorAll('[data-stack-container]').forEach(b=>b.onclick=()=>{const c=resource(b.dataset.stackContainer);if(c){state.selected=c.id;navigate('containers');}});
  bindContainerOverview(stack);
  if(stack.kind==='swarm')bindSwarm(stack);
}
function bindStackFilters(){
  document.querySelector('#stack-search')?.addEventListener('input',e=>{stackQuery=e.target.value;const pos=e.target.selectionStart;render();const input=document.querySelector('#stack-search');input.focus();input.setSelectionRange(pos,pos);});
  document.querySelectorAll('[data-stack-kind]').forEach(b=>b.onclick=()=>{stackKind=b.dataset.stackKind;stackChecked.clear();render();});
}
