const stackActionLabel={start:'启动',stop:'停止',restart:'重启',remove_stopped:'移除已停止容器',remove:'移除服务'};
function confirmStackAction(stack,action,scope){
  const swarm=stack.kind==='swarm';
  const affected=swarm?scope:scope.filter(c=>c.state===(['start','remove_stopped'].includes(action)?'stopped':'running'));
  if(!affected.length)return;
  const label=swarm&&action==='restart'?'滚动重启':stackActionLabel[action];
  const modal=openDialog(`${label}：${stack.name}`,`<p>影响 ${affected.length} 个${swarm?'服务':'容器'}</p><ul class="impact-list">${affected.map(c=>`<li>${esc(c.name)}</li>`).join('')}</ul><p class="dialog-explainer">${action==='remove'?'移除服务及其任务，保留网络、卷、配置和秘密。':action==='remove_stopped'?'删除容器可写层，卷、镜像和网络保留。':action==='stop'?'服务将中断。启用自动删除的容器可能被删除。':action==='restart'?(swarm?'按照服务的更新策略滚动重启任务。':'服务将短暂中断。'):'启动现有服务容器，不创建或更新部署。'}</p>`,`<button class="button close-modal">取消</button><button class="button ${action==='start'?'primary':'danger'}" id="confirm-stack-action">${label}</button>`);
  modal.querySelector('#confirm-stack-action').onclick=()=>{
    for(const c of affected){tasks.unshift({id:++taskId,title:`${label} ${c.name}`,status:'success',detail:'示例操作已完成',time:'刚刚'});if(!swarm)c.state=action==='stop'?'stopped':'running';else if(action==='restart')c.update='updating';}
    if(['remove','remove_stopped'].includes(action))stack.services=stack.services.filter(c=>!affected.includes(c));
    if(!stack.services.length)stackFixtures.splice(stackFixtures.indexOf(stack),1);
    stackChecked.clear();modal.close();render();toast('示例操作完成');
  };
}
function closeStackMenu(){document.querySelector('#stack-context-menu')?.remove();}
function openStackMenu(stack,index,trigger,point){
  closeStackMenu();const checked=stack.services.filter(c=>stackChecked.has(stackToken(stack,c)));
  const scope=index==='all'?(checked.length?checked:stack.services):[stack.services[Number(index)]];
  const actions=index==='all'?[stack.kind==='swarm'?'remove':'remove_stopped']:stack.kind==='swarm'?['restart','remove']:['start','stop','restart','remove_stopped'];
  const menu=document.createElement('div');menu.id='stack-context-menu';menu.className='env-context-menu';menu.setAttribute('role','menu');
  menu.innerHTML=actions.map(a=>`<button role="menuitem" data-action="${a}" ${stack.kind==='compose'&&scope.every(c=>c.state!==(['start','remove_stopped'].includes(a)?'stopped':'running'))?'disabled':''}>${stack.kind==='swarm'&&a==='restart'?'滚动重启':stackActionLabel[a]}…</button>`).join('')+(index==='all'?'':'<hr><button role="menuitem" data-copy="name">复制服务名称</button><button role="menuitem" data-copy="image">复制镜像名称</button>');
  document.body.append(menu);const rect=trigger.getBoundingClientRect();menu.style.left=Math.min(point?.x??rect.right-menu.offsetWidth,window.innerWidth-menu.offsetWidth-8)+'px';menu.style.top=Math.min(point?.y??rect.bottom+4,window.innerHeight-menu.offsetHeight-8)+'px';
  menu.querySelectorAll('[data-action]').forEach(b=>b.onclick=()=>{closeStackMenu();confirmStackAction(stack,b.dataset.action,scope);});
  menu.querySelectorAll('[data-copy]').forEach(b=>b.onclick=()=>{copyText(scope[0][b.dataset.copy]);closeStackMenu();});
  menu.querySelector('button:not(:disabled)')?.focus();
  menu.onkeydown=e=>{if(e.key==='Escape'){closeStackMenu();trigger.focus();}if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();const buttons=[...menu.querySelectorAll('button:not(:disabled)')];const i=buttons.indexOf(document.activeElement);buttons[(i+(e.key==='ArrowDown'?1:buttons.length-1))%buttons.length].focus();}};
}
document.addEventListener('pointerdown',e=>{if(!e.target.closest('#stack-context-menu,[data-stack-menu]'))closeStackMenu();});
