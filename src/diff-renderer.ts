import {pairedLines,type DiffLine,type Hunk} from './diff';
import {escapeHtml as e} from './html';

function cell(line:DiffLine|null,right:boolean){
  return `<div class="code-cell ${line?.kind??'blank'}"><span class="line-no">${line?(right?line.new:line.old)??'':''}</span><code ${line?`data-syntax-side="${right?'after':'before'}" data-syntax-line="${right?line.new:line.old}"`:''}>${e(line?.text??'')}</code></div>`;
}
export function renderDiffHunks(hunks:Hunk[],split:boolean,actions?:(index:number)=>string){
  return hunks.map(hunk=>`<section class="hunk"><div class="hunk-header"><code>${e(hunk.header)}</code>${actions?.(hunk.index)??''}</div>${split?`<div class="split-lines">${pairedLines(hunk.lines).map(([before,after])=>`<div class="split-row">${cell(before,false)}${cell(after,true)}</div>`).join('')}</div>`:`<div class="unified-lines">${hunk.lines.map(line=>`<div class="code-line ${line.kind}"><span class="line-no">${line.old??''}</span><span class="line-no">${line.new??''}</span><span class="sign">${line.kind==='add'?'+':line.kind==='remove'?'−':' '}</span><code data-syntax-side="${line.kind==='remove'?'before':'after'}" data-syntax-line="${line.kind==='remove'?line.old:line.new}">${e(line.text)}</code></div>`).join('')}</div>`}</section>`).join('');
}
