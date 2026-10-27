import {escapeHtml as e} from './html';

export function fileLabel(path:string){
  const slash=path.lastIndexOf('/');
  return `<span class="file-path">${e(path.slice(slash+1))}${slash>=0?`<small>${e(path.slice(0,slash))}</small>`:''}</span>`;
}
