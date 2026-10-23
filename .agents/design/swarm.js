/* Swarm review fixtures. Mutations are confined to this page. */
const swarmStackFixtures=[{name:'shop',kind:'swarm',services:[
  {name:'shop_web',image:'nginx:alpine',mode:'Replicated',replicas:3,ports:'8080 → 80/tcp',update:'completed',tasks:[{containerId:'8a72d4f1c036'+'a'.repeat(52),id:'task-web-1-fixture',node:'node-1',slot:1,state:'running',desired:'running'},{containerId:'8a72d4f1c037'+'a'.repeat(52),id:'task-web-2-fixture',node:'node-2',slot:2,state:'running',desired:'running'},{id:'task-web-3-fixture',node:'',slot:3,state:'pending',desired:'running'},...Array.from({length:9},(_,i)=>({id:`history-task-${i}`,node:'node-3',slot:3,state:'failed',desired:'shutdown'}))]},
  {name:'shop_agent',image:'grafana/agent:latest',mode:'Global',replicas:null,ports:'',update:'completed',tasks:[{id:'task-agent-1',node:'node-1',slot:0,state:'running',desired:'running'},{id:'task-agent-2',node:'node-2',slot:0,state:'running',desired:'running'}]},
  {name:'shop_worker',image:'ghcr.io/example/worker:2.4',mode:'Replicated',replicas:2,ports:'',update:'paused',tasks:[{id:'task-worker-1',node:'node-1',slot:1,state:'running',desired:'running'},{id:'task-worker-2',node:'node-2',slot:2,state:'rejected',desired:'running'}]},
  {name:'shop_cache',image:'redis:7-alpine',mode:'Replicated',replicas:1,ports:'',update:'completed',tasks:[{id:'task-cache-1',node:'node-2',slot:1,state:'running',desired:'running'}]},
  {name:'shop_preview',image:'ghcr.io/example/api:dev',mode:'Replicated',replicas:0,ports:'8081 → 3000/tcp',update:null,tasks:[]}
]}];
function bindSwarm(stack){
  document.querySelectorAll('[data-swarm-scale]').forEach(b=>b.onclick=()=>{
    const s=stack.services[Number(b.dataset.swarmScale)];
    const modal=openDialog(`调整副本数：${s.name}`,`<form id="swarm-scale-form" class="env-form"><div class="stack-scale-summary"><span>运行中 <strong>${stackRunning(s)}</strong></span><span>当前目标 <strong>${s.replicas}</strong></span></div><label class="field"><span>目标副本数</span><input id="swarm-replicas" type="number" min="0" max="10000" step="1" required value="${s.replicas}"></label><p class="dialog-explainer">设为 0 将停止全部副本。任务由 Swarm 调度。</p></form>`,`<button class="button close-modal">取消</button><button id="scale-submit" class="button primary" type="submit" form="swarm-scale-form">更新副本数</button>`);
    const input=modal.querySelector('input'),submit=modal.querySelector('#scale-submit');
    const sync=()=>{const zero=input.value.trim()==='0';submit.className='button '+(zero?'danger':'primary');submit.textContent=zero?'停止全部副本':'更新副本数';};input.oninput=sync;sync();
    modal.querySelector('form').onsubmit=e=>{e.preventDefault();s.replicas=Number(input.value);tasks.unshift({id:++taskId,title:`更新副本数 ${s.name}`,status:'success',detail:'示例配置已更新',time:'刚刚'});modal.close();render();};
  });
}
