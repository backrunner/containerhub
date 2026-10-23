function addEnvironmentDialog(){
  let mode='docker';
  const modal=openDialog('添加环境','',`<button class="button close-modal">取消</button><button class="button primary" id="add-env-submit">添加</button>`);
  const content=modal.querySelector('.dialog-content');
  const draw=()=>{
    content.innerHTML=`<div class="env-form"><div class="stack-add-tabs"><button class="button ${mode==='docker'?'selected':''}" data-add-mode="docker">Docker</button><button class="button ${mode==='agent'?'selected':''}" data-add-mode="agent">远程 Agent</button></div>${mode==='docker'?'<label class="field"><span>名称</span><input id="add-env-name" placeholder="例如 开发环境"></label><label class="field"><span>Docker context</span><input id="add-env-context" value="default"></label><label class="field"><span>CLI 路径</span><input id="add-env-cli" placeholder="自动发现 Docker CLI"></label><p class="dialog-explainer">使用已有 context 的连接与认证配置。</p>':'<label class="field"><span>绑定配置</span><input id="add-env-profile" placeholder="已绑定的控制端配置目录"></label><p class="dialog-explainer">导入已完成配对的 Agent，保留原绑定与密钥。</p>'}<p class="form-error" id="add-env-error" hidden></p></div>`;
    content.querySelectorAll('[data-add-mode]').forEach(b=>b.onclick=()=>{mode=b.dataset.addMode;draw();});
  };draw();
  modal.querySelector('#add-env-submit').onclick=()=>{
    const name=modal.querySelector('#add-env-name')?.value.trim(),context=modal.querySelector('#add-env-context')?.value.trim(),path=modal.querySelector('#add-env-profile')?.value.trim();
    if(mode==='docker'?(!name||!/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$/.test(context)):!path.startsWith('/')){const error=modal.querySelector('#add-env-error');error.textContent=mode==='docker'?'请填写名称和有效的 Docker context。':'请选择绑定配置的绝对路径。';error.hidden=false;return;}
    modal.close();toast('表单已通过检查 · 设计预览不连接运行时');
  };
}

function renderDockerEnvironment(t){
  return `<header class="host-heading"><div class="host-title"><h2>${esc(t.name)}</h2><span class="env-status">已连接</span></div><button class="icon-button bordered" data-env-edit="${t.id}" aria-label="编辑 ${esc(t.name)}">${icon('Settings2')}</button></header><p class="host-description">Docker Engine · ${esc(t.context)}</p><section class="env-section"><div class="env-section-heading"><h3>主机资源</h3></div><p class="host-description">此连接未提供主机采样</p></section>${renderEnvironmentContainers(t)}<section class="env-section"><div class="env-section-heading"><h3>连接信息</h3></div><dl class="connection-facts"><div><dt>Docker context</dt><dd>${esc(t.context)}</dd></div><div><dt>CLI 路径</dt><dd>${esc(t.cli||'/opt/homebrew/bin/docker')}</dd></div></dl></section>`;
}
