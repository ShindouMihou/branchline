import type {Tokens} from './syntax';

export function applySourceTokens(root:ParentNode,tokens:Tokens){
  root.querySelectorAll<HTMLElement>('code[data-syntax-line]').forEach(code=>{
    const side=code.dataset.syntaxSide;
    if(side!=='before'&&side!=='after')return;
    const token=tokens[side]?.[Number(code.dataset.syntaxLine)-1];
    if(token&&token.text===code.textContent)code.innerHTML=token.html;
  });
  root.querySelectorAll<HTMLElement>('span[data-token-color]').forEach(token=>{
    const color=token.dataset.tokenColor;
    if(color&&/^#[a-f\d]{6,8}$/i.test(color))token.style.color=color;
  });
}
