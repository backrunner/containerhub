const selectedImages = new Set();
function renderImages() {
  images.forEach(im=>im.used=containers.filter(c=>c.image===`${im.name}:${im.tag}`).length);
  return `${resourceToolbar('搜索镜像名称或标签…')}${selectedImages.size?`<div class="batch-bar"><strong>已选 ${selectedImages.size} 项</strong><button class="button small" id="clear-images">取消选择</button><button class="button small" id="copy-images">${icon('Copy')}复制引用</button><button class="button small danger" id="delete-images" ${isOffline()?'disabled':''}>${icon('Trash2')}删除所选</button></div>`:''}<div class="table-wrap resource-table-wrap"><table class="resource-table" aria-label="镜像列表"><thead><tr><th class="check-cell"><input type="checkbox" id="select-images" aria-label="全选当前筛选的镜像"></th><th>镜像 / 标签</th><th>平台</th><th>大小</th><th>使用情况</th><th style="text-align:right">操作</th></tr></thead><tbody>${images.map((im,i)=>`<tr class="${selectedImages.has(i)?'selected':''}"><td class="check-cell"><input type="checkbox" data-check-image="${i}" aria-label="选择镜像 ${esc(im.name)}" ${selectedImages.has(i)?'checked':''}></td><td><button class="resource-name" data-image="${i}">${esc(im.name)}</button><span class="resource-sub">${esc(im.tag)}</span></td><td class="mono muted">linux/arm64</td><td class="numeric">${esc(im.size)}</td><td>${im.used?im.used+' 个容器':'未使用'}</td><td style="text-align:right"><button class="button small" data-image="${i}">详情</button></td></tr>`).join('')}</tbody></table></div><div class="list-meta"><span>${images.length} 个本地镜像</span></div>`;
}
function bindImages() {
  const all=document.querySelector('#select-images');if(!all)return;
  const visible=()=>[...document.querySelectorAll('[data-check-image]')].filter(b=>!b.closest('tr').hidden);
  const sync=()=>{const items=visible();const checked=items.filter(b=>selectedImages.has(+b.dataset.checkImage)).length;all.checked=items.length>0&&checked===items.length;all.indeterminate=checked>0&&checked<items.length;all.disabled=!items.length;};
  sync();
  document.querySelector('#generic-search')?.addEventListener('input',sync);
  const rerender=()=>{const q=document.querySelector('#generic-search').value;render();const input=document.querySelector('#generic-search');input.value=q;input.dispatchEvent(new Event('input'));};
  document.querySelectorAll('[data-check-image]').forEach(b=>b.onchange=()=>{b.checked?selectedImages.add(+b.dataset.checkImage):selectedImages.delete(+b.dataset.checkImage);rerender();});
  all.onchange=()=>{visible().forEach(b=>all.checked?selectedImages.add(+b.dataset.checkImage):selectedImages.delete(+b.dataset.checkImage));rerender();};
  document.querySelector('#clear-images')?.addEventListener('click',()=>{selectedImages.clear();rerender();});
  document.querySelector('#copy-images')?.addEventListener('click',()=>copyText([...selectedImages].map(i=>`${images[i].name}:${images[i].tag}`).join('\n')));
  document.querySelector('#delete-images')?.addEventListener('click',()=>{
    const unused=[...selectedImages].filter(i=>!images[i].used), used=selectedImages.size-unused.length;
    const modal=openDialog('核对所选镜像',`<p class="dialog-explainer">${unused.length} 项可删除，${used} 项仍被容器引用，将跳过。删除前会重新核对引用关系。</p><ul class="impact-list">${[...selectedImages].map(i=>`<li>${esc(images[i].name)}:${esc(images[i].tag)}<small>${images[i].used?'跳过 · 仍在使用':'可删除'}</small></li>`).join('')}</ul>`,`<button class="button close-modal">取消</button><button class="button danger" id="confirm-image-delete" ${unused.length?'':'disabled'}>删除 ${unused.length} 项</button>`);
    document.querySelector('#confirm-image-delete').onclick=()=>{images=images.filter((_,i)=>!unused.includes(i));selectedImages.clear();modal.close();render();toast('示例镜像已删除');};
  });
}
