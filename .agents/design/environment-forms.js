let environmentTestSequence=0;
function environmentConnectionTest(id) {
  const t=envTarget(id);if(!t)return;
  const modal=openDialog('检查连接',`<div class="connection-test-target">${icon(t.transport==='local'?'Laptop':'Server')}<div><strong>${esc(t.name)}</strong><code>${esc(t.transport==='local'?'本机 CLI':t.transport==='relay'?t.relay:t.endpoint)}</code></div></div><div class="connection-test-steps" id="connection-check-result" aria-live="polite"></div><p class="env-footnote">设计示例，不会发起网络请求或访问本机运行时。</p>`,`<button class="button close-modal">关闭</button><button class="button primary" id="check-environment-again">重新检查</button>`);
  modal.classList.add('environment-check-dialog');
  const check=()=>{
    const token=++environmentTestSequence;modal.dataset.connectionTest=String(token);
    const result=modal.querySelector('#connection-check-result'),button=modal.querySelector('#check-environment-again');
    result.innerHTML=`<div>${icon('LoaderCircle','spin')}正在检查目标与绑定身份…</div>`;button.disabled=true;
    setTimeout(()=>{
      if(!modal.open||modal.dataset.connectionTest!==String(token)||!modal.querySelector('#connection-check-result'))return;
      const localOffline=t.transport==='local'&&environmentIsOffline(t);
      const failed=localOffline||(t.id==='qa'&&!t.needsCheck);
      result.innerHTML=failed?`<div class="failed">${icon('CircleAlert')}${localOffline?'运行时服务未启动':'目标主机不可达'}<small>${localOffline?'返回主机详情后可启动本机服务':'连接超时，未进入身份验证阶段'}</small></div>${localOffline?'':`<div class="pending">${icon('ShieldCheck')}绑定身份核对<small>尚未执行</small></div>`}`:`<div>${icon('CircleCheck')}目标可连接<small>${t.transport==='local'?'CLI 版本 1.6.0':t.latency+' ms · '+transportLabel(t)}</small></div>${t.transport!=='local'?`<div>${icon('ShieldCheck')}绑定身份一致<small>沿用原 Agent 指纹，未重新绑定</small></div>`:''}<div>${icon('CircleCheck')}运行时响应正常<small>示例状态已更新</small></div>`;
      button.disabled=false;
      if(!failed){t.offline=false;t.needsCheck=false;t.lastSeen='刚刚';if(state.scenario==='offline'){state.scenario='normal';document.querySelector('#scenario').value='normal';}render();}
    },700);
  };
  modal.querySelector('#check-environment-again').onclick=check;check();
}
function environmentIdentityDialog(id) {
  const t=envTarget(id);if(!t?.fingerprint)return;
  const modal=openDialog('绑定身份',`<div class="env-binding-note">${icon('ShieldCheck')}<div><strong>${esc(t.name)}</strong><p>每次连接均使用已固定的 Agent 公钥核对身份。直连与中继共用同一绑定。</p></div></div><label class="field"><span>Agent 公钥指纹</span><textarea readonly rows="3" id="env-public-fingerprint">${t.fingerprint}</textarea></label><dl class="identity-facts"><div><dt>工作空间</dt><dd>个人工作空间</dd></div><div><dt>此设备权限</dt><dd>读取 · 日志 · 容器与镜像管理</dd></div><div><dt>绑定时间</dt><dd>${t.paired}</dd></div></dl>`,`<span>示例公钥指纹 · 不含私钥</span><button class="button close-modal">关闭</button><button class="button" id="copy-agent-fingerprint">${icon('Copy')}复制指纹</button>`);
  modal.querySelector('#copy-agent-fingerprint').onclick=()=>copyText(t.fingerprint);
}
function environmentContainersDialog(id,filter='all') {
  const t=envTarget(id);if(!t)return;
  const rows=environmentRows(t).filter(c=>filter==='all'||c.state===filter||c.health===filter);
  const health={healthy:'健康',unhealthy:'不健康',unchecked:'未配置检查',excluded:'—'};
  const modal=openDialog(`容器 · ${t.name}`,`<div class="env-dialog-summary">${rows.length} 个容器${environmentIsOffline(t)?' · 断开前记录':' · 示例状态'}</div><div class="env-container-table"><table aria-label="环境容器概览"><thead><tr><th>容器</th><th>运行状态</th><th>健康检查</th></tr></thead><tbody>${rows.length?rows.map(c=>`<tr><td>${icon('Container')}<strong>${esc(c.name)}</strong></td><td>${statusPill(c.state)}</td><td><span class="health-text ${c.health}">${health[c.health]}</span></td></tr>`).join(''):'<tr><td colspan="3" class="env-table-empty">没有符合条件的容器</td></tr>'}</tbody></table></div>`,`<button class="button close-modal">关闭</button>`);
  modal.classList.add('environment-containers-dialog');
}
