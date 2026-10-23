/* The expanded service view contains actual containers, never scheduler history. */
const stackInspectedContainers = new Map([
  ['8a72d4f1c036'+'a'.repeat(52), {name:'shop_web.1.fx83pa',state:'running',health:'健康',cpu:'2.4%',memory:'48 MiB',limit:'512 MiB',ports:'',image:'nginx:alpine'}]
]);
function stackContainerOverview(s) {
  if(!s.tasks) return [{id:'demo-'+['web-api','postgres','redis','minio','nginx-proxy','worker','sandbox','mailpit'].indexOf(s.name),local:{name:s.name,state:s.state,health:s.health,cpu:s.state==='running'?'2.5%':'—',memory:s.state==='running'?'48 MiB':'—',limit:'512 MiB',ports:s.ports}}];
  const seen=new Set();
  return s.tasks.filter(t=>!['shutdown','remove'].includes(t.desired)&&/^[a-f0-9]{64}$/i.test(t.containerId||''))
    .sort((a,b)=>a.slot-b.slot||a.node.localeCompare(b.node)||a.id.localeCompare(b.id))
    .filter(t=>!seen.has(t.containerId)&&seen.add(t.containerId))
    .map(t=>({id:t.containerId,node:t.node,local:stackInspectedContainers.get(t.containerId)}));
}
function renderContainerOverview(stack,s,i) {
  const items=stackContainerOverview(s),pages=Math.max(1,Math.ceil(items.length/4));
  const page=Math.min(stackContainerPages.get(stackToken(stack,s))||0,pages-1);
  return `<div class="stack-instances"><div class="container-overview-header"><span>${items.length} 个容器</span>${s.truncated?'<span>采集不完整</span>':''}${s.tasks&&s.ports?`<span class="container-service-ports">服务端口 ${esc(s.ports)}</span>`:''}${pages>1?`<span>${page+1} / ${pages}</span>${stackIconButton('ChevronLeft','上一页容器',`data-container-page="${page-1}" data-service="${i}"`,page===0)}${stackIconButton('ChevronRight','下一页容器',`data-container-page="${page+1}" data-service="${i}"`,page+1>=pages)}`:''}</div>${items.length?'<div class="container-overview-columns"><span>名称 / ID</span><span>状态</span><span>CPU</span><span>内存</span><span>端口</span><span></span><span></span></div>':''}${items.slice(page*4,page*4+4).map(c=>{
    const v=c.local,shortId=c.id.slice(0,12),detail=c.node?'节点 '+c.node.slice(0,12):(v?shortId:'节点未提供');
    return `<article class="container-quick-card"><div class="container-quick-main"><div class="container-quick-identity"><strong>${esc(v?.name||shortId)}</strong><small title="${esc(c.id)}">${v&&c.node?`<span>${esc(shortId)}</span>`:''}<span>${esc(detail)}</span></small></div><div class="container-quick-state"><span class="${v?.state==='running'?'success':'muted'}">${v?v.state==='running'?'运行中':'已停止':'未采集'}</span><small class="${v?.health==='不健康'?'danger':''}">${v?v.state==='running'?esc(v.health):'—':'详情不可用'}</small></div>${stackIconButton('Copy','复制容器 ID',`data-copy-container="${esc(c.id)}"`)}${stackIconButton('ArrowUpRight',v?'查看容器详情':'当前连接未采集此容器的详情',`data-overview-inspect="${esc(c.id)}" data-service="${i}"`,!v)}</div><div class="container-quick-metrics"><div><small>CPU</small><strong>${v?.cpu||'—'}</strong></div><div><small>内存</small><strong>${v?.memory||'—'}${v?.limit?`<sub>/ ${esc(v.limit)}</sub>`:''}</strong></div><div><small>端口</small><strong title="${esc(v?.ports||'')}">${esc(v?.ports||'—')}</strong></div></div></article>`;
  }).join('')||'<div class="container-overview-empty">暂无可查看的容器</div>'}</div>`;
}
function bindContainerOverview(stack) {
  document.querySelectorAll('[data-copy-container]').forEach(b=>b.onclick=()=>copyText(b.dataset.copyContainer));
  document.querySelectorAll('[data-container-page]').forEach(b=>b.onclick=()=>{stackContainerPages.set(stackToken(stack,stack.services[Number(b.dataset.service)]),Number(b.dataset.containerPage));render();});
  document.querySelectorAll('[data-overview-inspect]').forEach(b=>b.onclick=()=>{
    const c=stackContainerOverview(stack.services[Number(b.dataset.service)]).find(c=>c.id===b.dataset.overviewInspect),v=c?.local;if(!v)return;
    if(!resource(v.name)) containers.push({id:v.name,image:v.image,status:v.state,cpu:2.4,memory:48,port:null,target:null,uptime:'2 小时',volume:null,project:stack.name,health:v.health});
    Object.assign(resource(v.name), {health:v.health,status:v.state});
    state.selected=v.name;navigate('containers');
  });
}
