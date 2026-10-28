import type {AutoApprovalConnection,AutoApprovalGrant,AutoApprovalScope,AutoApprovalPreview,AutoApprovalOperation} from './types/auto-approval';
import {approvalOverview,approvalEditor,approvalReview,canReviewRule} from './auto-approval-view';
import './auto-approval.css';
export {autoApprovalBanner,autoApprovalSummary,approvalOperations} from './auto-approval-view';
export function createAutoApprovalSettings(options:{render:()=>void;run:(fn:()=>Promise<unknown>)=>void;call:(request:object)=>Promise<unknown>;root:()=>string|null;pickRepository:()=>Promise<string|null>}){
  let editing=false,reviewing=false,confirmed=false;
  let prepared:AutoApprovalPreview|null=null;
  const scope:AutoApprovalScope={clientId:'',repository:'',allRepositories:false,sessionWide:false,operations:[],remote:'origin',branch:'',githubRepository:'',base:'main',minutes:null};
  let connections:AutoApprovalConnection[]=[];
  async function preview(){
    confirmed=false;prepared=null;
    prepared=await options.call({op:'auto_approval_preview',request:{...scope,operations:scope.sessionWide?[]:scope.operations,confirmed:false}}) as AutoApprovalPreview;
    reviewing=true;options.render();
  }
  function render(grants:AutoApprovalGrant[],clients:AutoApprovalConnection[],enabled:boolean,busy:boolean){
    connections=clients.filter(c=>c.active);
    if(!editing)return approvalOverview(grants,connections,enabled,busy);
    return `<section class="ai-auto-settings"><div class="ai-auto-editor-heading"><h3>${reviewing?'Review your rule':'New rule'}</h3><span>${reviewing?'2':'1'} of 2</span></div>${reviewing?approvalReview(scope,connections,prepared,confirmed,busy):approvalEditor(scope,connections,busy)}</section>`;
  }
  function bind(){
    const on=(id:string,fn:()=>void)=>document.getElementById(id)?.addEventListener('click',fn);
    on('ai-auto-add',()=>{Object.assign(scope,{clientId:connections.length===1?connections[0].id:'',repository:options.root()??'',allRepositories:false,sessionWide:false,operations:[],remote:'origin',branch:'',githubRepository:'',base:'main',minutes:null});editing=true;reviewing=false;confirmed=false;prepared=null;options.render()});
    on('ai-auto-cancel',()=>{editing=false;options.render()});
    on('ai-auto-review',()=>{if(canReviewRule(scope))options.run(preview)});
    on('ai-auto-edit',()=>{reviewing=false;confirmed=false;prepared=null;options.render()});
    on('ai-auto-enable',()=>{const snapshot=prepared;if(!confirmed||!snapshot)return;options.run(async()=>{await options.call({op:'auto_approval_grant',request:{...scope,operations:scope.sessionWide?[]:scope.operations,confirmed:true,previewDigest:snapshot.digest}});editing=false;reviewing=false;confirmed=false;})});
    for(const id of ['ai-auto-pause','ai-auto-pause-toolbar'])on(id,()=>options.run(()=>options.call({op:'auto_approval_pause'})));
    document.querySelectorAll<HTMLButtonElement>('[data-auto-remove]').forEach(button=>button.onclick=()=>options.run(()=>options.call({op:'auto_approval_remove',id:button.dataset.autoRemove})));
    document.querySelectorAll<HTMLButtonElement>('[data-auto-mode]').forEach(button=>button.onclick=()=>{scope.sessionWide=button.dataset.autoMode==='true';confirmed=false;prepared=null;options.render()});
    document.querySelectorAll<HTMLButtonElement>('[data-auto-mode]').forEach(button=>button.addEventListener('keydown',event=>{
      if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(event.key))return;
      event.preventDefault();scope.sessionWide=event.key==='Home'?false:event.key==='End'?true:!scope.sessionWide;options.render();
      document.querySelector<HTMLButtonElement>(`[data-auto-mode="${scope.sessionWide}"]`)?.focus({preventScroll:true});
    }));
    document.querySelectorAll<HTMLInputElement>('[data-auto-operation]').forEach(input=>input.onchange=()=>{const op=input.dataset.autoOperation as AutoApprovalOperation;scope.operations=input.checked?[...scope.operations,op]:scope.operations.filter(v=>v!==op);options.render()});
    document.querySelectorAll<HTMLInputElement|HTMLSelectElement>('[data-auto-field]').forEach(input=>input.addEventListener(input.tagName==='SELECT'?'change':'input',()=>{
      const field=input.dataset.autoField;
      if(field==='allRepositories'){
        if(input.value==='browse'){options.run(async()=>{try{const repository=await options.pickRepository();if(repository){scope.repository=repository;scope.allRepositories=false;confirmed=false;prepared=null;}}finally{options.render();}});return;}
        scope.allRepositories=input.value==='true';confirmed=false;prepared=null;options.render();return;
      }
      if(field==='minutes'){scope.minutes=input.value?Number(input.value):null;confirmed=false;prepared=null;options.run(preview);return;}
      if(field==='clientId'||field==='repository'||field==='remote'||field==='branch'||field==='githubRepository'||field==='base')scope[field]=input.value;
      if(input.tagName==='SELECT')options.render();
      else {const review=document.getElementById('ai-auto-review') as HTMLButtonElement|null;if(review)review.disabled=!canReviewRule(scope);}
    }));
    document.getElementById('ai-auto-confirm')?.addEventListener('change',event=>{confirmed=(event.target as HTMLInputElement).checked;const button=document.getElementById('ai-auto-enable') as HTMLButtonElement|null;if(button)button.disabled=!confirmed||!prepared;});
  }
  return {render,bind};
}
