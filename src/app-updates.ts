import {invoke,isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {escapeHtml as e} from './html';
import {renderAppUpdates} from './app-update-view';
import type {AppUpdateState} from './types/app-updates';

export function createAppUpdates(render:()=>void,beforeOpen:()=>void=()=>{}) {
  let state:AppUpdateState={revision:0,currentVersion:'',automatic:true,phase:'idle',available:null,downloaded:0,total:null,lastCheckedAt:null,error:null};
  let open=false,started=false,saving=false;
  let origin:HTMLElement|null=null;
  const busy=()=>['checking','downloading','installing'].includes(state.phase);
  const apply=(next:AppUpdateState)=>{if(next.revision<state.revision)return;state=next;render();};
  const call=async(request:object)=>{
    try {apply(await invoke<AppUpdateState>('app_update',{request}));}
    catch(error){state={...state,phase:'error',error:String(error)};render();}
  };
  const show=(check=false)=>{
    if(!open)origin=document.activeElement instanceof HTMLElement?document.activeElement:null;
    beforeOpen();open=true;render();
    document.querySelector<HTMLElement>('.app-updates-modal')?.focus({preventScroll:true});
    if(check)void call({op:'check',manual:true});
  };
  const close=()=>{open=false;render();origin?.focus({preventScroll:true});};
  return {
    isOpen:()=>open,
    show,
    toolbar:()=>state.available?`<button id="app-update-available" class="app-update-badge" title="Branchline ${e(state.available.version)} is available"><i data-lucide="arrow-up" aria-hidden="true"></i> App update</button>`:'',
    settings:()=>`<button id="app-updates-settings" class="app-updates-settings"><span>App updates</span><span>${state.currentVersion?`Version ${e(state.currentVersion)}`:'Check for updates'}<i data-lucide="chevron-right" aria-hidden="true"></i></span></button>`,
    render:()=>!open?'':renderAppUpdates(state,saving),
    bind:()=>{
      document.getElementById('app-update-available')?.addEventListener('click',()=>show());
      document.getElementById('app-updates-settings')?.addEventListener('click',()=>show());
      document.getElementById('app-updates-close')?.addEventListener('click',close);
      document.querySelectorAll<HTMLAnchorElement>('.app-update-notes [data-markdown-url]').forEach(link=>link.onclick=event=>{
        event.preventDefault();
        void invoke('open_markdown_link',{url:link.dataset.markdownUrl}).catch(error=>{state={...state,error:String(error)};render();});
      });
      document.getElementById('app-updates-check')?.addEventListener('click',()=>void call({op:'check',manual:true}));
      document.getElementById('app-updates-install')?.addEventListener('click',()=>{if(!busy()&&state.available)void call({op:'install',version:state.available.version});});
      document.getElementById('app-updates-automatic')?.addEventListener('change',event=>{
        const automatic=(event.target as HTMLInputElement).checked;
        saving=true;render();
        void call({op:'preferences',automatic}).finally(()=>{saving=false;render();if(automatic)void call({op:'check',manual:false});});
      });
      document.querySelector('.app-updates-modal')?.addEventListener('keydown',event=>{
        const key=event as KeyboardEvent;
        if(key.key==='Escape'){key.preventDefault();key.stopPropagation();close();}
        if(key.key==='Tab'){
          const controls=[...document.querySelectorAll<HTMLElement>('.app-updates-modal button:not(:disabled),.app-updates-modal input:not(:disabled),.app-updates-modal summary,.app-updates-modal a')];
          const first=controls[0],last=controls.at(-1);
          if(key.shiftKey&&(document.activeElement===first||document.activeElement?.matches('.app-updates-modal'))){key.preventDefault();last?.focus();}
          else if(!key.shiftKey&&document.activeElement===last){key.preventDefault();first?.focus();}
        }
      });
    },
    start:()=>{
      if(started||!isTauri())return;
      started=true;
      const subscriptions=[listen<AppUpdateState>('branchline-app-update',event=>apply(event.payload)),listen('check-for-app-updates',()=>show(true))];
      const check=()=>{void call({op:'check',manual:false});};
      const timer=window.setInterval(check,15*60*1000);
      void Promise.all(subscriptions).then(()=>call({op:'status'})).then(check).catch(()=>{});
      window.addEventListener('focus',check);
      window.addEventListener('pagehide',()=>{clearInterval(timer);window.removeEventListener('focus',check);void Promise.all(subscriptions).then(unlisten=>unlisten.forEach(stop=>stop()));},{once:true});
    },
  };
}
