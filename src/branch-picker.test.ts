// @vitest-environment happy-dom
import {it,expect,vi} from 'vitest';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/plugin-dialog',()=>({open:vi.fn(),save:vi.fn()}));
vi.mock('@tauri-apps/plugin-clipboard-manager',()=>({writeText:vi.fn(async()=>{})}));
it('opens from prefetched branches, anchors an action menu and preserves cached rows during failed reload',async()=>{
  document.body.innerHTML='<div id="app"></div>';localStorage.clear();localStorage.setItem('branchline.repo','/repo');
  const repo={root:'/repo',name:'Example',branch:'main',branchRef:'refs/heads/main',head:'a'.repeat(40),stateToken:'snapshot',files:[],stagedToken:'',remotes:['origin'],tracking:{upstream:'origin/main',ahead:0,behind:0,remote:'origin',merge:'main'},operation:null};
  const rows=[{reference:'refs/heads/main',name:'main',oid:repo.head,upstream:'origin/main',remote:false,current:true}];
  let resolve!: (value:typeof rows)=>void;let reads=0;let open!: (value:typeof repo)=>void;
  invoke.mockImplementation(async(command,{request}={})=>{
    if(command==='frontend_ready')return null;
    if(command==='ai_request')return {enabled:false,pending:0,clients:[],plans:[],selectedPlan:null};
    if(command.startsWith('plugin:'))return null;
    if(request.op==='open_quick')return new Promise(resolve=>{open=resolve});
    if(request.op==='open'||request.op==='status_details'||request.op==='status')return repo;
    if(request.op==='branches'){reads++;if(reads===1)return new Promise(done=>{resolve=done});if(reads===2)throw Error('temporary read error');return rows;}
    if(request.op==='revision')return {root:'/repo',revision:'same',watching:true};
    return [];
  });
  await import('./main');
  expect(document.querySelector('.startup-workspace')).not.toBeNull();expect(document.querySelector('.welcome')).toBeNull();open(repo);
  await vi.waitFor(()=>expect(document.querySelector('.shell')?.getAttribute('aria-busy')).toBe('false'));
  document.getElementById('branches')!.click();expect(document.querySelector('.branch-loading')?.textContent).toContain('Loading branches');
  resolve(rows);await vi.waitFor(()=>expect(document.querySelector('[data-select-branch]')).not.toBeNull());
  expect(reads).toBe(1);(document.querySelector('[data-select-branch]') as HTMLButtonElement).click();
  expect(document.querySelector('.branch-action-menu[role=menu]')).not.toBeNull();expect(document.querySelector('.branch-actions')).toBeNull();expect(document.querySelector('[data-branch-action="rename"]')).not.toBeNull();
  const first=document.activeElement;document.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));expect(document.activeElement).not.toBe(first);
  document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));expect(document.querySelector('.branch-action-menu')).toBeNull();expect(document.getElementById('branch-filter')).not.toBeNull();
  (document.querySelector('[data-select-branch]') as HTMLButtonElement).click();document.getElementById('branch-filter')!.dispatchEvent(new Event('pointerdown',{bubbles:true}));expect(document.querySelector('.branch-action-menu')).toBeNull();
  document.getElementById('dialog-close')!.click();document.getElementById('branches')!.click();expect(document.querySelector('[data-select-branch]')).not.toBeNull();expect(reads).toBe(1);
  document.getElementById('branch-refresh')!.click();await vi.waitFor(()=>expect(document.querySelector('.dialog-note')?.textContent).toContain('cached list'));expect(document.querySelector('[data-select-branch]')).not.toBeNull();
  document.getElementById('branch-refresh')!.click();await vi.waitFor(()=>expect(reads).toBe(3));expect(document.querySelector('[data-select-branch]')).not.toBeNull();
  window.dispatchEvent(new Event('pagehide'));
});
