/* Container pagination; selection and inspector remain stable across pages. */
function containerPage() {
  const rows=visibleContainers(), total=rows.length;
  const pages=Math.max(1,Math.ceil(total/state.pageSize));
  state.containerPage=Math.min(state.containerPage,pages-1);
  const start=state.containerPage*state.pageSize;
  return {items:rows.slice(start,start+state.pageSize),total,start,pages};
}
function renderPagination(total) {
  const {start,pages}=containerPage();
  const chevron=right=>`<svg class="icon" aria-hidden="true" viewBox="0 0 24 24"><path d="${right?'m9 6 6 6-6 6':'m15 6-6 6 6 6'}"/></svg>`;
  return `<nav class="container-pagination" aria-label="容器分页"><span class="pagination-range">${total?start+1:0}–${Math.min(total,start+state.pageSize)} / 共 ${total} 个${isOffline()?'（上次同步）':''}</span><select class="filter-select" id="container-page-size" aria-label="每页数量">${[5,10,20,50].map(n=>`<option value="${n}" ${n===state.pageSize?'selected':''}>${n} 条 / 页</option>`).join('')}</select><button class="icon-button" id="container-page-prev" aria-label="上一页" ${state.containerPage===0?'disabled':''}>${chevron(false)}</button><span class="pagination-number" aria-live="polite">${state.containerPage+1} / ${pages}</span><button class="icon-button" id="container-page-next" aria-label="下一页" ${state.containerPage===pages-1?'disabled':''}>${chevron(true)}</button></nav>`;
}
function bindPagination() {
  $('#container-page-size')?.addEventListener('change',e=>{state.pageSize=Number(e.target.value);state.containerPage=0;render();});
  $('#container-page-prev')?.addEventListener('click',()=>{state.containerPage--;render();$('#container-page-prev').focus();});
  $('#container-page-next')?.addEventListener('click',()=>{state.containerPage++;render();$('#container-page-next').focus();});
}
