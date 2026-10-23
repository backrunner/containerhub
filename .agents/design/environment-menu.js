let environmentMenuTrigger;
function closeEnvironmentContextMenu(restore=false) {
  document.querySelector('#environment-context-menu')?.remove();
  if(restore&&environmentMenuTrigger?.isConnected)environmentMenuTrigger.focus();
}
function openEnvironmentContextMenu(id,point,trigger) {
  const t=envTarget(id);if(!t)return;
  closeEnvironmentContextMenu();environmentMenuTrigger=trigger;
  const menu=document.createElement('div');menu.id='environment-context-menu';menu.className='env-context-menu';menu.setAttribute('role','menu');menu.setAttribute('aria-label',`${t.name} 环境操作`);
  const item=(action,glyph,label,disabled=false,danger=false)=>`<button role="menuitem" data-env-action="${action}" ${disabled?'disabled':''} class="${danger?'danger':''}">${icon(glyph)}<span>${label}</span></button>`;
  menu.innerHTML=`<div class="env-context-title">${esc(t.name)}</div>${item('inspect','LayoutDashboard','查看主机概览')}${item('activate','ArrowRight',id===reviewEnvironment?'当前工作环境':'切换到此环境',id===reviewEnvironment||environmentIsOffline(t))}<hr>${item('edit','Settings2','编辑环境…')}${item('copy','Copy',t.transport==='local'?'复制主机名':'复制连接地址')}${item('check','RefreshCw','检查连接')}${t.transport!=='local'?`<hr>${item('remove','Trash2','移除环境…',false,true)}`:''}`;
  document.body.append(menu);
  const bounds=menu.getBoundingClientRect();menu.style.left=`${Math.max(8,Math.min(point.x,innerWidth-bounds.width-8))}px`;menu.style.top=`${Math.max(8,Math.min(point.y,innerHeight-bounds.height-8))}px`;
  menu.querySelectorAll('[data-env-action]').forEach(b=>b.onclick=()=>{
    const action=b.dataset.envAction;closeEnvironmentContextMenu(true);
    if(action==='inspect')inspectEnvironment(id);
    if(action==='activate')selectReviewEnvironment(id);
    if(action==='edit')environmentEditDialog(id);
    if(action==='copy')copyText(t.transport==='local'?t.host:t.transport==='relay'?t.relay:t.endpoint);
    if(action==='check')environmentConnectionTest(id);
    if(action==='disconnect'){t.offline=true;t.lastSeen='刚刚';render();toast(`${t.name} 的示例连接已断开，主机上的容器保持运行。`);}
    if(action==='remove')removeEnvironmentDialog(id);
  });
  menu.onkeydown=e=>{
    const buttons=[...menu.querySelectorAll('button:not(:disabled)')],index=buttons.indexOf(document.activeElement);
    if(e.key==='Escape'){e.preventDefault();closeEnvironmentContextMenu(true);return;}
    if(e.key==='Tab'){closeEnvironmentContextMenu(true);return;}
    if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){e.preventDefault();buttons[e.key==='Home'?0:e.key==='End'?buttons.length-1:(index+(e.key==='ArrowDown'?1:buttons.length-1))%buttons.length].focus();}
  };
  menu.querySelector('button:not(:disabled)')?.focus();
}
function removeEnvironmentDialog(id) {
  const t=envTarget(id);if(!t||t.transport==='local')return;
  const modal=openDialog(`移除 ${t.name}？`,`<p class="dialog-explainer">从此设备的环境列表移除该连接。不会停止主机上的容器，也不会解除 Agent 的所有者绑定。</p>${id===reviewEnvironment?'<p class="dialog-explainer">这是当前工作环境，移除后将返回演示工作空间。</p>':''}`,`<button class="button close-modal">取消</button><button class="button danger" id="confirm-remove-environment">移除环境</button>`);
  modal.querySelector('#confirm-remove-environment').onclick=()=>{reviewTargets.splice(reviewTargets.findIndex(x=>x.id===id),1);if(reviewEnvironment===id)reviewEnvironment='demo';if(inspectedEnvironment===id)inspectedEnvironment='demo';modal.close();syncEnvironmentChrome();render();toast('已移除示例环境；未操作真实主机或绑定。');};
}
document.addEventListener('pointerdown',e=>{if(!e.target.closest('#environment-context-menu,[data-env-menu]'))closeEnvironmentContextMenu();});
window.addEventListener('resize',()=>closeEnvironmentContextMenu());
