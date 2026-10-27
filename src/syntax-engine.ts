import {createHighlighterCore} from 'shiki/core';
import {createJavaScriptRegexEngine} from 'shiki/engine/javascript';
import {bundledLanguages} from 'shiki/langs';
import theme from '@shikijs/themes/github-dark-dimmed';
const highlighter=createHighlighterCore({themes:[theme],langs:[],engine:createJavaScriptRegexEngine()});
const escape=(value:string)=>value.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
export async function highlightSource(source:string,language:string):Promise<{text:string;html:string}[]>{
  if(source.length>250_000||source.split('\n').length>6000||source.split('\n').some(line=>line.length>10000))return [];
  const load=bundledLanguages[language as keyof typeof bundledLanguages];if(!load)return [];
  const instance=await highlighter;if(!instance.getLoadedLanguages().includes(language))await instance.loadLanguage(load);
  return instance.codeToTokens(source,{lang:language,theme:'github-dark-dimmed'}).tokens.map(line=>({text:line.map(token=>token.content).join(''),html:line.map(token=>{
    const color=token.color&&/^#[a-f\d]{6,8}$/i.test(token.color)?` data-token-color="${token.color}"`:'';
    return `<span${color}>${escape(token.content)}</span>`;
  }).join('')}));
}
