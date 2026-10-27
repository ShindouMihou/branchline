// @vitest-environment happy-dom
import {it,expect,vi} from 'vitest';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/plugin-dialog',()=>({open:vi.fn(),save:vi.fn()}));
vi.mock('@tauri-apps/plugin-clipboard-manager',()=>({writeText:vi.fn(async()=>{})}));
it('shows images by default with a local code switch and keeps binary diff metadata available',async()=>{
  document.body.innerHTML='<div id="app"></div>';localStorage.clear();localStorage.setItem('branchline.repo','/repo');
  const repo={root:'/repo',name:'Images',branch:'main',branchRef:'refs/heads/main',head:'a'.repeat(40),stateToken:'snapshot',files:[{path:'icon.svg',index:'M',worktree:' '},{path:'pixel.png',index:'M',worktree:' '}],stagedToken:'staged',remotes:[],tracking:{ahead:0,behind:0},operation:null};
  const svg='<svg xmlns="http://www.w3.org/2000/svg"><rect width="2"/></svg>';
  const image={dataUrl:'data:image/svg+xml;base64,PHN2Zy8+',bytes:6};
  invoke.mockImplementation(async(command,{request}={})=>{
    if(command==='frontend_ready'||command.startsWith('plugin:'))return null;
    if(command==='ai_request')return {enabled:false,pending:0,clients:[],plans:[],selectedPlan:null};
    if(['open_quick','status_details','status'].includes(request.op))return repo;
    if(request.op==='branches')return [];
    if(request.op==='diff')return {patch:request.path.endsWith('.png')?'diff --git a/pixel.png b/pixel.png\nGIT binary patch\nliteral 4\ntest':'diff --git a/icon.svg b/icon.svg\n--- a/icon.svg\n+++ b/icon.svg\n@@ -1 +1 @@\n-<svg/>\n+'+svg};
    if(request.op==='image_preview')return {before:image,after:image};
    if(request.op==='source_preview')return {before:'<svg/>',after:svg};
    if(request.op==='revision')return {root:'/repo',revision:'same',watching:true};return [];
  });
  await import('./main');await vi.waitFor(()=>expect(document.querySelectorAll('.image-canvas img')).toHaveLength(2));
  expect(document.querySelector('.diff-pane .asset-view-switch')).not.toBeNull();(document.querySelector('[data-asset-mode="code"]') as HTMLButtonElement).click();
  expect(document.querySelector('.asset-view-switch button[aria-pressed="true"]')?.textContent).toBe('Code');expect(document.querySelector('.diff-content')?.textContent).toContain('<svg');expect(document.querySelector('.diff-content code svg')).toBeNull();
  (document.querySelector('[data-file="1"]') as HTMLButtonElement).click();await vi.waitFor(()=>expect(document.querySelectorAll('.image-canvas img')).toHaveLength(2));expect(document.querySelector('.asset-view-switch button[aria-pressed="true"]')?.textContent).toBe('Image');
  (document.querySelector('[data-asset-mode="code"]') as HTMLButtonElement).click();expect(document.querySelector('.binary-diff')?.textContent).toContain('GIT binary patch');
  window.dispatchEvent(new Event('pagehide'));
});
