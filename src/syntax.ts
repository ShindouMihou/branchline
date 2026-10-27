import {languages} from './language-info';
export {languages};
const aliases=new Map(languages.flatMap(language=>[language.id,...language.aliases].map(alias=>[alias.toLowerCase(),language.id] as const)));
const extensions:Record<string,string>={h:'c',hpp:'cpp',hh:'cpp',cc:'cpp',cxx:'cpp',cs:'csharp',fs:'fsharp',kt:'kotlin',kts:'kotlin',m:'objective-c',mm:'objective-cpp',rs:'rust',py:'python',rb:'ruby',ps1:'powershell',sh:'shellscript',zsh:'shellscript',bash:'shellscript',yml:'yaml',svg:'xml',plist:'xml',vue:'vue',svelte:'svelte',jsx:'jsx',tsx:'tsx',mjs:'javascript',cjs:'javascript',mts:'typescript',cts:'typescript',mdx:'mdx',jsonc:'jsonc',properties:'properties',tf:'terraform',gradle:'groovy',r:'r',ex:'elixir',exs:'elixir',erl:'erlang',hrl:'erlang',clj:'clojure',cljs:'clojure',cljc:'clojure',jl:'julia',hs:'haskell',lhs:'haskell',ml:'ocaml',mli:'ocaml',vb:'vb',proto:'proto',asm:'asm',s:'asm',bat:'bat',cmd:'bat'};
export function detectLanguage(path:string):string|null{
  const file=path.split('/').at(-1)?.toLowerCase()??'';
  const special:Record<string,string>={dockerfile:'docker',containerfile:'docker',makefile:'make',gnumakefile:'make','cmakelists.txt':'cmake','.gitignore':'gitignore','.gitattributes':'git-attributes','.editorconfig':'ini','.bashrc':'shellscript','.zshrc':'shellscript','.env':'dotenv'};
  const extension=file.split('.').at(-1)??'';const candidate=special[file]??extensions[extension]??extension;
  return aliases.get(candidate.toLowerCase())??null;
}
export interface TokenLine {text:string;html:string}
export interface Tokens {before:TokenLine[];after:TokenLine[];notice?:string}
let worker:Worker|undefined,id=0,workerUnavailable=false;
interface Pending {resolve:(tokens:Tokens)=>void;source:{before:string|null;after:string|null};language:string}
const pending=new Map<number,Pending>();
async function fallback(source:Pending['source'],language:string):Promise<Tokens>{
  if((source.before?.length??0)+(source.after?.length??0)>80_000)return {before:[],after:[]};
  try {const {highlightSource}=await import('./syntax-engine');return {before:await highlightSource(source.before??'',language),after:await highlightSource(source.after??'',language)}}catch(error){return {before:[],after:[],notice:`Syntax highlighting unavailable: ${String(error)}`}}
}
export function tokenize(source:Pending['source'],language:string):Promise<Tokens>{
  if(typeof Worker==='undefined'||workerUnavailable)return fallback(source,language);
  if(!worker){
    try{worker=new Worker(new URL('./syntax-worker.ts',import.meta.url),{type:'module'});}catch{workerUnavailable=true;return fallback(source,language)}
    worker.onmessage=event=>{const item=pending.get(event.data.id);if(item){if(event.data.error)void fallback(item.source,item.language).then(item.resolve);else item.resolve(event.data);pending.delete(event.data.id)}};
    worker.onerror=()=>{workerUnavailable=true;for(const item of pending.values())void fallback(item.source,item.language).then(item.resolve);pending.clear();worker?.terminate();worker=undefined;};
  }
  const request=++id;return new Promise(resolve=>{pending.set(request,{resolve,source,language});worker!.postMessage({id:request,language,...source});setTimeout(()=>{const item=pending.get(request);if(!item)return;pending.delete(request);workerUnavailable=true;void fallback(item.source,item.language).then(item.resolve)},4000)});
}
