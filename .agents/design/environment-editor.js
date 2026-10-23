function environmentEditDialog(id){
  const t=envTarget(id);if(!t)return;
  const docker=t.provider==='docker',remote=t.transport!=='local'&&!docker,local=t.id!=='demo'&&!remote&&!docker;
  let policy=t.policy||'auto',interval=t.refresh||2;
  const choices=(values,current,key)=>`<div class="env-choice-row">${values.map(([value,label])=>`<button type="button" class="button ${value===current?'selected':''}" data-${key}="${value}">${label}</button>`).join('')}</div>`;
  const modal=openDialog('编辑环境',`<form class="env-form" id="environment-edit-form"><label class="field"><span>名称</span><input id="env-edit-name" value="${esc(t.name)}" required maxlength="60"></label><label class="field"><span>备注</span><input id="env-edit-notes" value="${esc(t.description||'')}" maxlength="240"></label>${remote?`<div class="field"><span>连接策略</span>${choices([['direct','直连'],['relay','中继'],['auto','自动']],policy,'env-policy')}</div><label class="field" id="env-direct-field"><span>直连地址</span><input id="env-edit-endpoint" value="${esc(t.endpoint||'')}"></label><label class="field" id="env-relay-field"><span>中继地址</span><input id="env-edit-relay" value="${esc(t.relay||'')}"></label><p class="dialog-explainer">保存后重新验证原 Agent 身份，绑定保持不变。</p>`:''}${docker?`<div class="field"><span>Docker context</span><div class="env-readonly">${esc(t.context)}</div></div>`:''}${local||docker?`<label class="field"><span>CLI 路径</span><input id="env-edit-cli" value="${esc(t.cli||'')}"></label>`:''}<div class="field"><span>采样间隔</span>${choices([['2','2 秒'],['5','5 秒'],['10','10 秒']],String(interval),'env-interval')}</div><p class="form-error" id="env-form-error" role="alert" hidden></p></form>`,`<button class="button close-modal">取消</button><button class="button primary" type="submit" form="environment-edit-form">保存</button>`);
  modal.classList.add('environment-edit-dialog');
  const sync=()=>{if(remote){modal.querySelector('#env-direct-field').hidden=policy==='relay';modal.querySelector('#env-relay-field').hidden=policy==='direct';}};sync();
  modal.querySelectorAll('[data-env-policy]').forEach(b=>b.onclick=()=>{policy=b.dataset.envPolicy;modal.querySelectorAll('[data-env-policy]').forEach(x=>x.classList.toggle('selected',x===b));sync();});
  modal.querySelectorAll('[data-env-interval]').forEach(b=>b.onclick=()=>{interval=Number(b.dataset.envInterval);modal.querySelectorAll('[data-env-interval]').forEach(x=>x.classList.toggle('selected',x===b));});
  modal.querySelector('form').onsubmit=e=>{
    e.preventDefault();const name=modal.querySelector('#env-edit-name').value.trim(),cli=modal.querySelector('#env-edit-cli')?.value.trim();
    const fail=text=>{const error=modal.querySelector('#env-form-error');error.hidden=false;error.textContent=text;};
    if(!name||reviewTargets.some(x=>x.id!==id&&x.name===name))return fail('请填写未被使用的环境名称');
    if(cli&&!cli.startsWith('/'))return fail('CLI 路径需为绝对路径');
    if(remote){
      const endpoint=modal.querySelector('#env-edit-endpoint').value.trim(),relay=modal.querySelector('#env-edit-relay').value.trim();
      const valid=value=>{try{const u=new URL(value);return u.protocol==='wss:'&&u.hostname&&!u.username&&!u.password&&!u.hash&&!u.search;}catch{return false;}};
      if((policy!=='relay'&&!valid(endpoint))||(policy!=='direct'&&!valid(relay)))return fail('请输入有效的 wss:// 地址');
      const changed=t.policy!==policy||t.endpoint!==endpoint||t.relay!==relay;
      Object.assign(t,{policy,endpoint,relay});if(changed)Object.assign(t,{offline:true,needsCheck:true,lastSeen:'配置修改前',transport:policy==='relay'?'relay':'direct'});
    }
    Object.assign(t,{name,description:modal.querySelector('#env-edit-notes').value,refresh:interval});if(cli!==undefined)t.cli=cli;
    modal.close();syncEnvironmentChrome();render();
  };
}
