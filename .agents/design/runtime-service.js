// Visual/interaction simulation only. No process, network or permission access.
let reviewServicePhase = 'idle';
function renderRuntimeService() {
  const target=inspectedTarget(),offline=environmentIsOffline(target);
  const phase=target.servicePhase||reviewServicePhase;
  const active = phase === 'starting';
  const uncertain = phase === 'unknown';
  const label = active ? '正在启动服务' : uncertain ? '启动结果待核对' : offline ? '服务未连接' : '服务可连接';
  return `<div class="runtime-service" aria-live="polite"><div><span class="runtime-service-label"><i class="status-dot ${offline?'':'online'}"></i>${label}</span>${offline?'<p>服务未启动。可启动本机运行时，或在终端完成首次安装。</p>':''}</div>${offline?uncertain?`<button class="button small" id="runtime-reconcile">${icon('RefreshCw')}核对结果</button>`:`<button class="button small primary" id="runtime-start" ${active?'disabled':''}>${icon('Play')}${active?'启动中…':'启动服务'}</button>`:''}</div>`;
}
function runtimeStartDialog() {
  const target=inspectedTarget();
  if (!environmentIsOffline(target) || target.transport!=='local' || (target.servicePhase||reviewServicePhase)!=='idle') return;
  const modal = openDialog('启动运行时服务', '<p class="dialog-explainer">启动当前环境的后台服务，完成后自动检查连接。此次不会主动启动已有容器。</p><p class="dialog-explainer">首次安装所需的内核下载和 macOS 权限，请在终端完成。此次启动不会自动安装内核或提升权限。</p>', '<button class="button close-modal">取消</button><button class="button primary" id="confirm-runtime-start">启动服务</button>');
  document.querySelector('#confirm-runtime-start').onclick = () => {
    if ((target.servicePhase||reviewServicePhase)!=='idle') return;
    const generation = state.generation;
    const environment = target.id;
    target.servicePhase = 'starting';
    const task = {id:taskId++,title:'启动运行时服务',status:'busy',detail:'正在检查服务连接…',time:'刚刚'};
    tasks.unshift(task);
    modal.close(); render();
    setTimeout(() => {
      if (generation !== state.generation || !envTarget(environment)) return;
      reviewServicePhase = 'idle';target.servicePhase='idle';target.offline=false;
      state.scenario = 'normal';
      document.querySelector('#scenario').value = 'normal';
      task.status = 'success'; task.detail = '示例服务已连接';
      render(); toast('示例服务已连接；未执行本机命令');
    }, 900);
  };
}
function bindRuntimeService() {
  document.querySelector('#runtime-start')?.addEventListener('click', runtimeStartDialog);
  document.querySelector('#runtime-reconcile')?.addEventListener('click', () => {
    toast('示例服务仍未连接，已保留结果待核对；未重复启动。');
  });
}
