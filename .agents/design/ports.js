/* All changes are in-memory simulations; no local runtime calls. */
function portMappings(c) { return c.ports || (c.port ? [{host:'127.0.0.1',host_port:c.port,container_port:c.target,protocol:'tcp'}] : []); }
function portEditor(id) {
  const c=resource(id);if(!c)return;
  const original=JSON.stringify(portMappings(c)), originalStatus=c.status;
  let rows=structuredClone(portMappings(c));
  const modal=openDialog('编辑端口映射',`<p class="dialog-explainer">${esc(id)} · 主机 → 容器</p><div id="port-editor"></div><p class="rebuild-warning">将停止并删除原容器，使用原镜像和新端口重建。<br>可写层数据会删除，挂载卷和目录保留。<br>重建后恢复原运行状态；失败不会回滚到原容器。</p><p class="form-error" id="port-error" role="alert"></p><p class="log-note">设计原型 · 重建仅影响示例数据</p>`,`<button class="button close-modal">取消</button><button class="button danger" id="rebuild-ports" ${isOffline()?'disabled':''}>重建</button>`);
  const draw=()=>{
    $('#port-editor').innerHTML=`<div class="port-grid port-head"><span>主机地址</span><span>主机端口</span><span>容器端口</span><span>协议</span><span></span></div><div class="port-editor-rows">${rows.map((p,i)=>`<div class="port-grid"><input data-port-field="host" data-row="${i}" aria-label="主机地址 ${i+1}" value="${esc(p.host)}"><input data-port-field="host_port" data-row="${i}" aria-label="主机端口 ${i+1}" inputmode="numeric" value="${p.host_port||''}"><input data-port-field="container_port" data-row="${i}" aria-label="容器端口 ${i+1}" inputmode="numeric" value="${p.container_port||''}"><select data-port-field="protocol" data-row="${i}" aria-label="协议 ${i+1}"><option ${p.protocol==='tcp'?'selected':''}>tcp</option><option ${p.protocol==='udp'?'selected':''}>udp</option></select><button class="icon-button bordered" data-remove-port="${i}" aria-label="删除第 ${i+1} 条端口映射">${icon('Trash2')}</button></div>`).join('')}</div><button class="button small" id="add-port" ${rows.length>=32?'disabled':''}>${icon('Plus')}添加映射</button>`;
    $$('[data-port-field]').forEach(input=>input.oninput=()=>{rows[+input.dataset.row][input.dataset.portField]=input.value;});
    $$('[data-remove-port]').forEach(b=>b.onclick=()=>{rows.splice(+b.dataset.removePort,1);draw();});
    $('#add-port').onclick=()=>{rows.push({host:'127.0.0.1',host_port:0,container_port:0,protocol:'tcp'});draw();};
  };draw();
  $('#rebuild-ports').onclick=()=>{
    const fail=message=>$('#port-error').textContent=message;
    if(isOffline())return fail('环境已离线，暂不能重建');
    if(resource(id)!==c || c.status!==originalStatus || JSON.stringify(portMappings(c))!==original)return fail('容器配置已改变，请重新打开后核对');
    const ports=rows.map(p=>({...p,host:p.host.trim(),host_port:Number(p.host_port),container_port:Number(p.container_port)}));
    const normalizeIP=host=>{
      if (/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(host) && host.split('.').every(n=>+n<=255)) return host;
      if (!host.includes(':') || /[\[\]\s%/]/.test(host)) return null;
      try { return new URL(`http://[${host}]/`).hostname.slice(1,-1); } catch { return null; }
    };
    if(rows.some(p=>![p.host_port,p.container_port].every(n=>/^\d+$/.test(String(n)))) || ports.some(p=>!normalizeIP(p.host)||![p.host_port,p.container_port].every(n=>Number.isInteger(n)&&n>=1&&n<=65535)))return fail('填写有效 IP 和 1–65535 的整数端口');
    const overlap=(a,b)=>{
      const x=normalizeIP(a.host),y=normalizeIP(b.host);
      return a.protocol===b.protocol&&a.host_port===b.host_port&&(x===y||x==='::'||y==='::'||x==='0.0.0.0'&&!y.includes(':')||y==='0.0.0.0'&&!x.includes(':'));
    };
    if(ports.some((p,i)=>ports.slice(0,i).some(q=>overlap(p,q)))||containers.some(other=>other!==c&&portMappings(other).some(q=>ports.some(p=>overlap(p,q)))))return fail('主机端口重复或与其他容器冲突');
    if(JSON.stringify(ports)===original)return fail('端口映射未改变');
    const replacement={...c,ports,port:ports[0]?.host_port||null,target:ports[0]?.container_port||null};
    containers.splice(containers.indexOf(c),1,replacement);
    tasks.unshift({id:taskId++,title:`重建 ${id}`,target:id,status:'success',detail:'已按新端口重建；原示例容器已删除',time:'刚刚'});
    modal.close();render();toast('已按新端口重建示例容器');
  };
}
