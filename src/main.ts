import logoUrl from '../assets/branchline-mark.svg';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { createIcons, FolderOpen, GitBranch, RefreshCw, GitCommitHorizontal, History, Files, Plus, Minus, Check, ChevronRight, Columns2, AlignLeft, Search, ArrowDown, ArrowUp, X, Circle, FileCode2, Settings2, CircleHelp, GitMerge, Pencil, Cherry, Undo2, Copy, ShieldCheck, Plug, Sparkles, Clock3, ChevronDown, ChevronUp, ChevronLeft, Terminal, ArrowRight, CircleDot, Info, FolderPlus, ArrowUpRight, Flag, GitPullRequest, Trash2, BookOpen } from 'lucide';
import { parseDiff } from './diff';
import {renderDiffHunks} from './diff-renderer';
import {applySourceTokens} from './source-tokens';
import { graphRows } from './log';
import { createAutoRefresh } from './auto-refresh';
import { appearanceKey, defaultAppearance, parseAppearance, applyAppearance, rowHeight, type Appearance } from './appearance';
import { createAiWorkspace } from './ai';
import { captureAiViewport, restoreAiViewport } from './ai-viewport';
import { copyText } from './clipboard';
import {parseLayout,layoutKey,togglePanel,bindPanelResizers,isPanelResizing,type PanelLayout} from './panel-layout';
import {createBranchCache,menuPosition} from './branch-cache';
import {detectLanguage,languages,tokenize,type Tokens} from './syntax';
import {createSkillSetup} from './skill-setup';
import './style.css';
interface FileEntry { path: string; old_path: string | null; index: string; worktree: string }
interface Repo { root: string; name: string; branch: string; branchRef: string; head: string; stateToken: string; revision?: string; watching?: boolean; files: FileEntry[]; lineStats?:{additions:number;deletions:number;binaryFiles:number;skippedFiles:number}|null; stagedToken: string; detailsPending?:boolean; remotes: string[]; tracking: { upstream: string | null; ahead: number; behind: number; remote: string | null; merge: string | null }; operation: string | null; operationResult?: string; backupRef?: string }
interface Commit { oid: string; short: string; author: string; timestamp: number; subject: string; parents: string[]; refs: string }
interface Branch { reference: string; name: string; oid: string; upstream: string; remote: boolean; current: boolean }
interface RewritePreview { token: string; plan: { kind: string; selected: string[]; affected: string[]; message: string; branch: string; head: string } }
type Dialog = { type: 'appearance' } | { type: 'branches' } | {type:'branch-action';kind:'rename'|'delete'|'push';source:Branch;head:string;name:string;remote:string;destination:string} | { type: 'network'; kind: string; remote: string; branch: string; head: string } | { type: 'rewrite'; kind: string; oids: string[]; preview: RewritePreview; message: string } | { type: 'commit-action'; kind: string; oid: string; head: string } | { type: 'create'; reference: string; head: string; name: string; sourceName?:string } | { type: 'abort'; kind: string };
interface ImageVersion {dataUrl:string;bytes:number}
interface ImagePreview {before:ImageVersion|null;after:ImageVersion|null;notice?:string}
const imageFile=(path:string)=>/\.(svg|png|jpe?g|gif|webp|avif|bmp|ico)$/i.test(path);
let assetMode:'image'|'code'='image',activePreviewFile='';
const mediaCache=new Map<Diff,ImagePreview|'loading'>();
const syntaxCache=new Map<Diff,Map<string,Tokens|'loading'>>();
const languageOverrides=new Map<string,string>();
interface Diff { patch: string; untracked?: string | null; notice?: string }
let appearance = parseAppearance(localStorage.getItem(appearanceKey));
applyAppearance(appearance);
let savedRepository='';try{savedRepository=isTauri()?localStorage.getItem('branchline.repo')??'':''}catch{}
let restoringRepository=!!savedRepository;let persistRepository=true;
const app = document.querySelector<HTMLDivElement>('#app')!;
let layout:PanelLayout;try{layout=parseLayout(localStorage.getItem(layoutKey))}catch{layout=parseLayout(null)}
const saveLayout=()=>{try{localStorage.setItem(layoutKey,JSON.stringify(layout))}catch{}};
let repo: Repo | null = null, tab: 'changes' | 'history' = 'changes', selected: { path: string; staged: boolean } | null = null;
let diff: Diff | null = null, commits: Commit[] = [], commit: Commit | null = null, commitFiles: string[] = [], historyPath = '';
let message = '', error = '', notification = '', busy = false, loadingDiff = false, split = true, filter = '', historyLoaded = false, sequence = 0;
let selectedBranchReference='',branchMenuOpen=false,branchLoadError='';
let branchMenuPoint={x:0,y:0,alignRight:true};
const branchCache=createBranchCache<Branch[]>(root=>api<Branch[]>({op:'branches',expected_root:root}));
let branches: Branch[] = [], dialog: Dialog | null = null, branchFilter = '', selectedOids = new Set<string>(), anchor = 0, logAll = true, logFilter = '', menu = false;
let previousFocus: HTMLElement | null = null;
let diffObserver:ResizeObserver|undefined;
let recoveryRef = "";
let updateMenu = false;
let menuPoint={x:0,y:0};
let historyRequest=0, historyPending=false;
const historyCache=new Map<string,{files:string[];path:string;diff:Diff|null}>();
const skillSetup=createSkillSetup(()=>render());
const ai=createAiWorkspace({render:()=>render(),root:()=>repo?.root??null,canPrompt:()=>!dialog&&!busy&&!historyPending&&!skillSetup.isOpen(),openSkills:()=>void skillSetup.show()});
const icons = { FolderOpen, GitBranch, RefreshCw, GitCommitHorizontal, History, Files, Plus, Minus, Check, ChevronRight, Columns2, AlignLeft, Search, ArrowDown, ArrowUp, X, Circle, FileCode2, Settings2, CircleHelp, GitMerge, Pencil, Cherry, Undo2, Copy, ShieldCheck, Plug, Sparkles, Clock3, ChevronDown, ChevronUp, ChevronLeft, Terminal, ArrowRight, CircleDot, Info, FolderPlus, ArrowUpRight, Flag, GitPullRequest, Trash2, BookOpen };
const icon = (name: string) => `<i data-lucide="${name}" aria-hidden="true"></i>`;
const esc = (s: unknown) => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!));
const api = <T>(request: object) => invoke<T>('request', { request });
const basename = (p: string) => p.split('/').at(-1) ?? p;
function date(ts: number) { return new Date(ts * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }); }
function render() {
  const active = document.activeElement as HTMLInputElement | HTMLTextAreaElement | null;
  const focusId = active?.id, start = active?.selectionStart, end = active?.selectionEnd;
  const scroll = app.querySelector('.diff-content')?.scrollTop ?? 0;
  const diffHorizontal=app.querySelector('.diff-content')?.scrollLeft??0;
  const sideOffsets=[...app.querySelectorAll<HTMLElement>('[data-diff-scroll]')].map(track=>track.scrollLeft);
  const oldCommit=app.querySelector<HTMLElement>('.log-details')?.dataset.commitOid;
  const fileFocus=active?.dataset.historyFile;
  const listScroll = app.querySelector('.file-list')?.scrollTop ?? 0;
  const logScroll = app.querySelector('.log-rows')?.scrollTop ?? 0;
  const commitListScroll=app.querySelector('.log-details')?.scrollTop??0;
  const aiViewport=captureAiViewport(app);
  const branchScroll=app.querySelector('.branch-list')?.scrollTop??0;
  const branchFocus=active?.closest('.branch-action-menu')?(active as HTMLButtonElement).dataset.branchAction:undefined;
  app.innerHTML = `<div class="shell" aria-busy="${busy}">
    <header class="topbar"><div class="brand"><span class="brand-mark"><img src="${logoUrl}" alt="" width="26" height="26"></span><span class="wordmark">branchline</span></div>
    <button id="open" class="repo-picker" title="${repo ? esc(repo.root)+' · Change repository (⌘O)' : 'Open repository (⌘O)'}">${icon('folder-open')}<span>${repo ? esc(repo.name) : restoringRepository?esc(basename(savedRepository)):'Open repository'}</span>${icon('chevron-right')}</button>
    ${repo ? `<button id="branches" class="branch" title="Switch branch">${icon('git-branch')} <span class="branch-name">${esc(repo.branch)}</span> ${icon('chevron-right')}</button>` : ''}
    <div class="top-actions">${repo ? `<div class="update-control"><div class="update-split"><button id="update" title="Fetch the latest commits and fast-forward this branch" ${busy?'disabled':''}>${icon('arrow-down')} Update${repo.tracking.behind?`<span class="action-count">${repo.tracking.behind}</span>`:''}</button><button id="update-options" title="Update options" aria-label="Update options" aria-haspopup="menu" aria-expanded="${updateMenu}" aria-controls="update-menu" ${busy?'disabled':''}>${icon('chevron-right')}</button></div>${updateMenu?updateOptions():''}</div><button data-network="push" title="Push your commits" ${busy?'disabled':''}>${icon('arrow-up')} Push${repo.tracking.ahead?`<span class="action-count">${repo.tracking.ahead}</span>`:''}</button>`:''}${ai.toolbar()}<button id="appearance" class="icon-button" title="Appearance" aria-label="Appearance settings">${icon('settings-2')}</button></div></header>
    ${error ? `<div class="banner error" role="alert"><span>${esc(error)}</span><button id="dismiss" aria-label="Dismiss error">${icon('x')}</button></div>` : ''}
    ${notification ? `<div class="banner success" role="status"><span>${icon('check')} ${esc(notification)}</span>${recoveryRef ? `<details class="recovery-details"><summary>Recovery details</summary><code>${esc(recoveryRef)}</code><button id="copy-recovery">Copy recovery ref</button></details>` : ''}<button id="dismiss-note" aria-label="Dismiss notification">${icon('x')}</button></div>` : ''}
    ${repo?.operation ? `<div class="banner operation"><span>A ${esc(repo.operation)} is in progress. Finish it in Git, or abort.</span><button id="abort-operation">Abort ${esc(repo.operation)}</button></div>` : ''}
    ${repo ? workspace() : restoringRepository?startupWorkspace():welcome()}
    ${dialog ? renderDialog() : ''}${ai.render()}${skillSetup.render()}
    <footer class="statusbar"><span>${icon('git-branch')} ${esc(repo?.branch ?? 'No repository')}</span><span>${busy ? 'Git operation in progress…' : loadingDiff ? 'Loading diff…' : repo ? changedLines(repo) : 'Ready'}</span><span class="status-right">${repo ? esc(repo.root) : 'Local repositories · Git staging workflow'}</span></footer>
  </div>`;
  createIcons({ icons, attrs: { width: 16, height: 16, 'stroke-width': 1.6 } });
  bind(); bindOperations();ai.bind();skillSetup.bind();bindPanelResizers(app,layout,saveLayout);
  restoreAiViewport(app,aiViewport);
  sizeDiffColumns();app.querySelectorAll<HTMLElement>('[data-diff-scroll]').forEach((track,index)=>{track.scrollLeft=sideOffsets[index]??0;track.dispatchEvent(new Event('scroll'));});diffObserver?.disconnect();const diffViewport=app.querySelector('.diff-content');if(diffViewport&&typeof ResizeObserver!=='undefined'){diffObserver=new ResizeObserver(sizeDiffColumns);diffObserver.observe(diffViewport);}
  const branchList=app.querySelector('.branch-list');if(branchList)branchList.scrollTop=branchScroll;
  const branchMenu=app.querySelector<HTMLElement>('.branch-action-menu');if(branchMenu){if(branchMenuPoint.alignRight){const index=branches.findIndex(b=>b.reference===selectedBranchReference),row=app.querySelector<HTMLElement>(`[data-select-branch="${index}"]`);if(row){const bounds=row.getBoundingClientRect();branchMenuPoint={x:bounds.right,y:bounds.top+Math.min(32,bounds.height),alignRight:true};}}const bounds=branchMenu.getBoundingClientRect();const position=menuPosition(branchMenuPoint,bounds,{width:window.innerWidth,height:window.innerHeight});branchMenu.style.left=`${position.left}px`;branchMenu.style.top=`${position.top}px`;if(branchFocus)[...branchMenu.querySelectorAll<HTMLButtonElement>('[data-branch-action]')].find(button=>button.dataset.branchAction===branchFocus)?.focus({preventScroll:true});}
  const context=app.querySelector<HTMLElement>('.log-action-menu');if(context){const bounds=context.getBoundingClientRect();context.style.left=Math.max(8,Math.min(menuPoint.x,window.innerWidth-bounds.width-8))+'px';context.style.top=Math.max(8,Math.min(menuPoint.y,window.innerHeight-bounds.height-8))+'px';}
  const content = app.querySelector('.diff-content'); if (content) {content.scrollTop = scroll;content.scrollLeft=diffHorizontal;}
  const list = app.querySelector('.file-list'); if (list) list.scrollTop = listScroll;
  const logRows = app.querySelector('.log-rows'); if (logRows) logRows.scrollTop=logScroll;
  const commitList=app.querySelector('.log-details');if(commitList)commitList.scrollTop=oldCommit===commit?.oid?commitListScroll:0;
  if(fileFocus&&oldCommit===commit?.oid)app.querySelector<HTMLElement>(`[data-history-file="${Number(fileFocus)}"]`)?.focus({preventScroll:true});
  void prepareFilePreview();
  if (focusId) { const el = document.getElementById(focusId) as HTMLInputElement | HTMLTextAreaElement | null; if (el) { el.focus({preventScroll:true}); if (start != null && end != null) el.setSelectionRange(start, end); } }
}
function sizeDiffColumns(){
  const viewport=app.querySelector<HTMLElement>('.diff-content');if(!viewport)return;
  viewport.style.setProperty('--diff-viewport',`${viewport.clientWidth}px`);
  const tracks=app.querySelectorAll<HTMLElement>('[data-diff-scroll]');
  tracks.forEach((track,index)=>{
    let width=track.clientWidth;
    viewport.querySelectorAll<HTMLElement>(`.code-cell:nth-child(${index+1}) code`).forEach(code=>width=Math.max(width,code.scrollWidth+46));
    (track.firstElementChild as HTMLElement).style.width=`${width}px`;
    track.onscroll=()=>viewport.style.setProperty(index?'--after-scroll':'--before-scroll',`${track.scrollLeft}px`);
  });
  viewport.onwheel=event=>{
    if(!tracks.length||(!event.deltaX&&!event.shiftKey))return;
    const bounds=viewport.getBoundingClientRect(),track=tracks[event.clientX<bounds.left+bounds.width/2?0:1];
    if(track.scrollWidth<=track.clientWidth)return;
    event.preventDefault();track.scrollLeft+=event.deltaX||event.deltaY;
  };
}
function changedLines(repo:Repo){
  const stats=repo.lineStats;
  if(!stats)return 'Counting changed lines…';
  const tip=`Across staged and unstaged changes, including untracked text files.${stats.binaryFiles?` ${stats.binaryFiles} binary file(s) excluded.`:''}${stats.skippedFiles?` ${stats.skippedFiles} large or unreadable file(s) excluded.`:''}`;
  return `<span class="line-summary" title="${esc(tip)}"><span class="lines-added" aria-label="${stats.additions} added lines">${icon('plus')}${stats.additions.toLocaleString()}</span><span class="lines-removed" aria-label="${stats.deletions} removed lines">${icon('minus')}${stats.deletions.toLocaleString()}</span>${stats.skippedFiles?`<span class="line-count-partial" aria-label="Partial line count">${icon('info')}</span>`:''}</span>`;
}
function startupWorkspace(){return `<div class="workbench" aria-busy="true"><main class="workspace startup-workspace" style="--sidebar-width:${layout.sidebarWidth}px"><aside class="sidebar ${layout.sidebarCollapsed?'is-collapsed':''}">${!layout.sidebarCollapsed?`<div class="tabs"><button disabled>Changes</button><button disabled>History</button></div><div class="startup-lines" aria-hidden="true"><span></span><span></span><span></span></div>`:''}</aside><section class="editor"><div class="editor-tab" role="status">Opening ${esc(basename(savedRepository))}…</div><div class="startup-editor"></div></section></main></div>`;}
async function finishOpening(root:string){
  const opening=repo;try{const details=await api<Repo>({op:'status_details',expected_root:root});if(repo?.root!==root||repo!==opening)return;repo=details;render();void prefetchBranches(true);}
  catch{if(repo?.root===root){error='Couldn’t finish reading the repository. Refresh to try again.';render()}}
}
function welcome() {
  return `<main class="welcome"><div class="welcome-symbol"><img src="${logoUrl}" alt="Branchline" width="54" height="54"></div><h1>Your repository.<br>Your next good commit.</h1><p>Review changes, stage precisely, and keep your history close.</p><button class="primary" id="welcome-open">${icon('folder-open')} Open a repository <kbd>⌘O</kbd></button><div class="open-path"><input id="repo-path" aria-label="Repository folder path" placeholder="Or enter a repository folder path"><button id="open-path">Open</button></div><div class="workflow"><span>${icon('files')} Review diffs</span><span>${icon('plus')} Stage hunks</span><span>${icon('git-commit-horizontal')} Commit with confidence</span></div>${!isTauri() ? '<p class="browser-note">Run the desktop app to connect to local repositories.</p>' : ''}</main>`;
}
function workspace() {
  return `<div class="workbench"><nav class="rail" aria-label="Tool windows"><button data-tab="changes" class="${tab === 'changes' ? 'active' : ''}" aria-label="Local changes" title="Local changes">${icon('git-commit-horizontal')}</button><button data-tab="history" class="${tab === 'history' ? 'active' : ''}" aria-label="History" title="History">${icon('history')}</button></nav>
    <main class="workspace" style="--sidebar-width:${layout.sidebarWidth}px"><aside class="sidebar ${layout.sidebarCollapsed?'is-collapsed':''}" aria-label="Files panel">${layout.sidebarCollapsed?`<button data-toggle-panel="sidebar" class="icon-button" aria-label="Expand files panel" title="Expand files panel">${icon('chevron-right')}</button><button data-tab="changes" class="icon-button" aria-label="Changes" title="Changes">${icon('files')}</button><button data-tab="history" class="icon-button" aria-label="History" title="History">${icon('history')}</button>`:`<div class="sidebar-title"><strong>${tab === 'changes' ? 'Changes' : 'History'}</strong><span class="subtle">${tab === 'changes' ? repo!.files.length : commits.length}</span></div>
    <div class="tabs"><button data-tab="changes" class="${tab === 'changes' ? 'active' : ''}">Changes <span class="tab-count" aria-label="${repo!.files.length} changed file${repo!.files.length===1?'':'s'}">${repo!.files.length}</span></button><button data-tab="history" class="${tab === 'history' ? 'active' : ''}">History</button><button data-toggle-panel="sidebar" class="icon-button panel-collapse" aria-label="Collapse files panel" title="Collapse files panel">${icon('chevron-left')}</button></div>
    ${tab === 'changes' ? changes() : historyDetails()}
    `}</aside>${!layout.sidebarCollapsed?`<div class="panel-resizer column-resizer" data-resize-panel="sidebar" role="separator" tabindex="0" aria-label="Resize files panel" aria-orientation="vertical" aria-valuemin="220" aria-valuemax="700" aria-valuenow="${Math.round(layout.sidebarWidth)}"></div>`:''}<section class="editor ${layout.logCollapsed?'log-collapsed':''} ${layout.diffCollapsed?'diff-collapsed':''}" style="--log-size:${layout.logRatio*100}%" aria-label="Review panels">${tab === 'history' ? (layout.logCollapsed?`<div class="collapsed-panel"><button data-toggle-panel="log" aria-label="Expand commit list">${icon('chevron-down')} Commit list <span>${commits.length}</span></button></div>`:history()) : ''}${tab==='history'&&!layout.logCollapsed&&!layout.diffCollapsed?`<div class="panel-resizer row-resizer" data-resize-panel="log" role="separator" tabindex="0" aria-label="Resize commit list and diff" aria-orientation="horizontal" aria-valuemin="15" aria-valuemax="85" aria-valuenow="${Math.round(layout.logRatio*100)}"></div>`:''}<section class="diff-pane ${layout.diffCollapsed?'is-collapsed':''}" aria-label="Diff viewer">${layout.diffCollapsed?`<div class="collapsed-panel"><button data-toggle-panel="diff" aria-label="Expand diff viewer">${icon('chevron-up')} Diff</button></div>`:editor()}</section></section></main></div>`;
}
function changes() {
  if(appearance.simplified && !repo!.files.length && !message) return `<div class="clean-state">${icon('check')}<h2>All clear</h2><p>Your working tree is clean.</p></div>`;
  const filtered = repo!.files.filter(f => f.path.toLowerCase().includes(filter.toLowerCase()));
  const staged = filtered.filter(f => f.index !== ' ' && f.index !== '?');
  const unstaged = filtered.filter(f => f.worktree !== ' ' || f.index === '?');
  return `<label class="search">${icon('search')}<input id="filter" value="${esc(filter)}" placeholder="Filter changed files" aria-label="Filter changed files"></label>
  <div class="file-list">${group('Unstaged', unstaged, false)}${group('Staged', staged, true)}${repo!.files.length === 0 ? '<div class="list-empty">Working tree is clean.<br><span>Your next change will appear here.</span></div>' : ''}</div>
  <div class="commit-box"><div class="commit-label">${icon('git-commit-horizontal')} Commit message <span>⌘↵</span></div><textarea id="message" placeholder="Describe what changed and why…" aria-label="Commit message" spellcheck="true">${esc(message)}</textarea><div class="commit-foot"><span>${repo!.files.filter(f => f.index !== ' ' && f.index !== '?').length} staged files</span><button id="commit" class="primary" ${busy || !message.trim() || !repo!.stagedToken ? 'disabled' : ''}>${icon('check')} Commit</button></div><p class="commit-help">Commits only staged changes. Git hooks and signing settings apply.</p></div>`;
}
function group(label: string, files: FileEntry[], staged: boolean) {
  if(appearance.simplified && !files.length) return '';
  return `<div class="group-heading">${icon('chevron-right')} ${label}<span>${files.length}</span></div>${files.length ? files.map(f => {
    const index = repo!.files.indexOf(f); const active = selected?.path === f.path && selected.staged === staged;
    const code = staged ? f.index : f.worktree;
    return `<div class="file-row ${active ? 'selected' : ''}"><button class="file-select" data-file="${index}" data-staged="${staged}" title="${esc(f.path)}" ${busy ? 'disabled' : ''}><span class="file-status ${code === 'D' ? 'deleted' : code === 'A' || code === '?' ? 'added' : ''}">${esc(code === '?' ? '+' : code)}</span><span class="file-path">${esc(basename(f.path))}<small>${esc(f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '')}</small></span></button><button class="file-action" data-stage="${index}" data-staged="${staged}" title="${staged ? 'Unstage' : 'Stage'} file" aria-label="${staged ? 'Unstage' : 'Stage'} ${esc(f.path)}" ${busy ? 'disabled' : ''}>${icon(staged ? 'minus' : 'plus')}</button></div>`;
  }).join('') : '<div class="group-empty">No changes</div>'}`;
}
function historyDetails() {
  if(!commit)return `<div class="list-empty"><strong>Choose a commit</strong><br><span>Select one in the commit list to see its details and changed files.</span></div>`;
  const when=new Date(commit.timestamp*1000).toLocaleString(undefined,{month:'short',day:'numeric',year:'numeric',hour:'numeric',minute:'2-digit'});
  return `<div class="commit-details log-details" data-commit-oid="${esc(commit.oid)}"><div class="commit-overview-label">${icon('git-commit-horizontal')} Selected commit</div><h2>${esc(commit.subject)}</h2><dl class="commit-metadata"><div><dt>Author</dt><dd>${esc(commit.author)}</dd></div><div><dt>Date</dt><dd>${esc(when)}</dd></div><div><dt>Commit</dt><dd><code title="${esc(commit.oid)}">${esc(commit.oid.slice(0,12))}</code><button id="copy-selected-commit" class="icon-button" title="Copy full commit hash" aria-label="Copy full commit hash">${icon('copy')}</button></dd></div></dl><div class="commit-files-heading"><h3>Files changed</h3><span>${commitFiles.length}</span></div><div class="commit-files">${commitFiles.map((path,index)=>`<button data-history-file="${index}" class="${path===historyPath?'selected':''}" title="${esc(path)}" aria-label="Show diff for ${esc(path)}">${icon('file-code-2')}<span><strong>${esc(basename(path))}</strong>${path.includes('/')?`<small>${esc(path.slice(0,path.lastIndexOf('/')))}</small>`:''}</span></button>`).join('')||'<p class="subtle">This commit has no changed files.</p>'}</div></div>`;
}
function history() {
  const visible=commits.map((c,i)=>({c,i})).filter(({c})=>(c.subject+' '+c.author+' '+c.oid+' '+c.refs).toLowerCase().includes(logFilter.toLowerCase()));
  const graphs=graphRows(commits); const graphWidth=Math.max(36,...graphs.map(g=>g.width*16+12));
  return `<section class="log-panel" aria-label="Git log"><div class="log-toolbar"><select id="log-scope" aria-label="History scope"><option value="all" ${logAll?'selected':''}>All branches</option><option value="current" ${!logAll?'selected':''}>Current branch</option></select><input id="log-filter" value="${esc(logFilter)}" placeholder="Search commits" aria-label="Search loaded commits"><button id="log-actions" aria-label="Commit actions" aria-haspopup="menu" aria-expanded="${menu}" ${!selectedOids.size||busy?'disabled':''}>Actions ${icon('chevron-down')}</button>${selectedOids.size>1?`<span class="selection-count">${selectedOids.size} selected</span>`:''}<span class="selection-help"><button class="icon-button" aria-label="Commit selection help" aria-describedby="selection-hint">${icon('circle-help')}</button><span id="selection-hint" role="tooltip">Shift-click selects a range. ⌘-click toggles a commit. Right-click opens actions.${logFilter?' Graph follows the unfiltered list.':''}</span></span><button data-toggle-panel="log" class="icon-button" aria-label="Collapse commit list" title="Collapse commit list">${icon('chevron-up')}</button></div>
    ${menu ? actionMenu() : ''}<div class="log-columns"><span style="width:${graphWidth}px">Graph</span><span>Commit</span><span>Author</span><span class="log-date">Date</span><span class="log-hash">Hash</span></div>
    <div class="log-rows" role="group" aria-label="Commit list">${visible.map(({c,i})=>{ const g=graphs[i];return `<button aria-pressed="${selectedOids.has(c.oid)}" class="log-row ${selectedOids.has(c.oid)?'selected':''}" data-commit="${i}" title="${esc(c.subject)}" ${busy?'disabled':''}><svg width="${graphWidth}" height="${rowHeight(appearance)}" viewBox="0 0 ${graphWidth} 36" preserveAspectRatio="none" aria-hidden="true">${g.incoming.map(l=>`<path d="M ${l*16+12} 0 V 18" class="lane lane-${g.color}"/>`).join('')}${g.edges.map(e=>`<path d="M ${e.from*16+12} 18 C ${e.from*16+12} 27 ${e.to*16+12} 27 ${e.to*16+12} 36" class="lane lane-${e.color}"/>`).join('')}<circle cx="${g.lane*16+12}" cy="18" r="3.5" class="node lane-${g.color}"/></svg><span class="log-subject">${c.refs?`<span class="ref-label">${esc(c.refs)}</span>`:''}${esc(c.subject)}</span><span class="log-author">${esc(c.author)}</span><span class="log-date">${date(c.timestamp)}</span><code class="log-hash">${esc(c.short)}</code></button>`;}).join('')}
    ${!commits.length?'<div class="list-empty">No commits yet.</div>':!visible.length?'<div class="list-empty">No matching commits in the loaded history.</div>':''}
    ${commits.length&&commits.length%100===0?'<button id="load-more" class="load-more">Load older commits</button>':''}</div></section>`;
}
function actionMenu() {
  const items=[['squash','git-merge','Squash selected commits…',selectedOids.size<2],['reword','pencil','Edit commit message…',selectedOids.size!==1],['cherry-pick','cherry','Cherry-pick…',selectedOids.size!==1],['revert','undo-2','Revert commit…',selectedOids.size!==1],['create','git-branch','New branch from commit…',selectedOids.size!==1],['copy','copy','Copy commit hash',selectedOids.size!==1]];
  return `<div class="log-action-menu" role="menu" aria-label="Commit actions">${items.map(([kind,glyph,label,disabled])=>`<button role="menuitem" data-log-action="${kind}" ${disabled?'disabled':''}>${icon(String(glyph))}<span>${label}</span></button>`).join('')}</div>`;
}

function editor() {
  const path = tab === 'changes' ? selected?.path : historyPath;
  if (!path) return `<button data-toggle-panel="diff" class="icon-button empty-diff-collapse" aria-label="Collapse diff viewer" title="Collapse diff viewer">${icon('chevron-down')}</button><div class="editor-empty">${icon(tab === 'changes' ? 'files' : 'history')}<h2>${tab === 'changes' ? 'Every change, in context' : 'Follow the history'}</h2><p>${tab === 'changes' ? 'Select a file to review its diff.' : 'Select a commit to browse its files and changes.'}</p><div class="empty-shortcuts"><span>Open repository <kbd>⌘O</kbd></span><span>Refresh <kbd>⌘R</kbd></span></div></div>`;
  const fileKey=repo!.root+':'+path;if(activePreviewFile!==fileKey){activePreviewFile=fileKey;assetMode='image';}
  const asset=imageFile(path),showImage=asset&&assetMode==='image';
  const language=languageOverrides.get(fileKey)??detectLanguage(path)??'text';
  const staged = selected?.staged ?? false;
  const oversized = (diff?.patch.length ?? 0) > 1_000_000 || (diff?.patch.match(/\n/g)?.length ?? 0) > 10000;
  const parsed = parseDiff(oversized ? '' : diff?.patch ?? '');
  if (oversized && diff) diff.notice = 'This diff exceeds the preview limit (1 MB or 10,000 lines). Use file-level staging.';
  const canHunk = tab === 'changes' && parsed.hunks.length > 0 && !/GIT binary patch|new file mode|deleted file mode|old mode/.test(diff?.patch ?? '');
  return `<div class="editor-tab ${asset?'asset-file':''}">${icon('file-code-2')} <span>${esc(basename(path))}</span><small>${tab === 'history' ? esc(commit?.short) : staged ? 'Staged' : 'Working tree'}</small><button data-toggle-panel="diff" class="icon-button panel-collapse" aria-label="Collapse diff viewer" title="Collapse diff viewer">${icon('chevron-down')}</button></div>
  <div class="diff-toolbar revision-toolbar ${split?'split-head':''}"><span class="revision-from">${tab === 'history' ? 'Parent revision' : staged ? 'HEAD' : 'Index'}</span><div class="revision-to"><span>${tab === 'history' ? `Commit ${esc(commit?.short)}` : staged ? 'Staged version' : 'Working tree'}</span><div class="diff-controls">${!showImage?`<select id="syntax-language" class="syntax-language" aria-label="Syntax highlighting language" title="Auto-detect or choose a language"><option value="auto" ${!languageOverrides.has(fileKey)?'selected':''}>Auto${detectLanguage(path)?`: ${languages.find(item=>item.id===detectLanguage(path))?.name??detectLanguage(path)}`:''}</option><option value="text" ${languageOverrides.get(fileKey)==='text'?'selected':''}>Plain text</option>${languages.map(item=>`<option value="${esc(item.id)}" ${languageOverrides.get(fileKey)===item.id?'selected':''}>${esc(item.name)}</option>`).join('')}</select>`:''}<span class="diff-count">${showImage?'Image comparison':`${parsed.hunks.length} ${parsed.hunks.length===1?'hunk':'hunks'}`}</span>${!showImage?`<button id="view-mode" class="quiet-control" title="${split?'Show a unified diff':'Show changes side by side'}" aria-label="${split?'Show unified diff':'Show side-by-side diff'}">${icon(split ? 'align-left' : 'columns-2')} ${split ? 'Unified' : 'Side by side'}</button>`:''}</div></div></div>
  ${asset?`<div class="asset-view-switch" aria-label="Image display mode"><button data-asset-mode="image" aria-pressed="${showImage}">Image</button><button data-asset-mode="code" aria-pressed="${!showImage}">Code</button></div>`:''}
  <div class="diff-content ${split&&!showImage?'split-view':''}" tabindex="0" aria-label="File diff">
  ${showImage?imageComparison(diff):loadingDiff ? '<div class="diff-notice">Loading diff…</div>' : diff?.untracked != null ? `<div class="diff-notice">${esc(diff.notice)}</div><pre class="untracked" data-syntax-untracked>${esc(diff.untracked)}</pre>` : parsed.hunks.length ? renderDiffHunks(parsed.hunks,split,canHunk?index=>`<button data-hunk="${index}" ${busy?'disabled':''}>${icon(staged?'minus':'plus')} ${staged?'Unstage':'Stage'} hunk</button>`:undefined) : asset&&diff?.patch&&!oversized?`<div class="binary-source-note">Binary image · Git diff details</div><pre class="binary-diff">${esc(diff.patch)}</pre>`:`<div class="diff-notice">${esc(diff?.notice ?? (diff?.patch.includes('GIT binary patch') ? 'Binary file. Use the file action to stage or unstage.' : diff?.patch ? 'Metadata-only change. Use the file action to stage or unstage.' : 'No textual changes to display.'))}</div>`}
  </div>${split&&!showImage&&parsed.hunks.length?`<div class="diff-x-scrollbars" aria-label="Horizontal diff scrolling"><div data-diff-scroll="before" tabindex="0" aria-label="Scroll previous version horizontally"><div></div></div><div data-diff-scroll="after" tabindex="0" aria-label="Scroll new version horizontally"><div></div></div></div>`:''}`;
}
function imageComparison(value:Diff|null){
  const preview=value?mediaCache.get(value):undefined;
  if(!preview||preview==='loading')return '<div class="image-status" role="status">Loading image preview…</div>';
  if(!preview.before&&!preview.after)return `<div class="image-status">${esc(preview.notice??'No image available. Choose Code to see the Git diff.')}</div>`;
  return `<div class="image-comparison">${(['before','after'] as const).map(side=>`<figure><figcaption>${side==='before'?'Before':'After'}${preview[side]?`<small>${Math.ceil(preview[side]!.bytes/1024)} KB</small>`:''}</figcaption><div class="image-canvas">${preview[side]?`<img src="${esc(preview[side]!.dataUrl)}" alt="${side==='before'?'Before':'After'} version" decoding="async">`:'<span>Not present</span>'}</div></figure>`).join('')}</div>`;
}
function applyTokens(tokens:Tokens){
  const picker=app.querySelector<HTMLElement>('#syntax-language');if(picker)picker.title=tokens.notice??'Auto-detect or choose a language';
  const content=app.querySelector('.diff-content');if(content)applySourceTokens(content,tokens);
  const untracked=app.querySelector<HTMLElement>('[data-syntax-untracked]');if(untracked&&tokens.after.map(line=>line.text).join('\n')===untracked.textContent){untracked.innerHTML=tokens.after.map(line=>line.html).join('\n');untracked.querySelectorAll<HTMLElement>('[data-token-color]').forEach(token=>token.style.color=token.dataset.tokenColor??'');}
}
async function prepareFilePreview(){
  const value=diff,root=repo?.root,path=tab==='history'?historyPath:selected?.path,oid=tab==='history'?commit?.oid:null,staged=tab==='changes'&&!!selected?.staged;
  if(!value||!root||!path)return;
  const current=()=>diff===value&&repo?.root===root&&(tab==='history'?historyPath:selected?.path)===path;
  if(imageFile(path)&&assetMode==='image'){
    if(mediaCache.has(value))return;mediaCache.set(value,'loading');while(mediaCache.size>4)mediaCache.delete(mediaCache.keys().next().value!);
    try{const preview=await api<ImagePreview>({op:'image_preview',path,staged,oid,expected_root:root});mediaCache.set(value,preview)}catch{mediaCache.set(value,{before:null,after:null,notice:'Image preview unavailable. Choose Code to inspect the diff.'})}
    while(mediaCache.size>1&&[...mediaCache.values()].reduce((sum,item)=>sum+(item==='loading'?0:(item.before?.dataUrl.length??0)+(item.after?.dataUrl.length??0)),0)>32_000_000){const oldest=[...mediaCache.keys()].find(key=>key!==value&&key!==diff);if(!oldest)break;mediaCache.delete(oldest);}
    if(current())render();return;
  }
  const language=languageOverrides.get(root+':'+path)??detectLanguage(path);if(!language||language==='text')return;
  let cache=syntaxCache.get(value);if(!cache){cache=new Map();syntaxCache.set(value,cache);while(syntaxCache.size>6)syntaxCache.delete(syntaxCache.keys().next().value!);}
  const cached=cache.get(language);if(cached==='loading')return;if(cached){applyTokens(cached);return}cache.set(language,'loading');
  try{const source=await api<{before:string|null;after:string|null}>({op:'source_preview',path,staged,oid,expected_root:root});const tokens=source.before===null&&source.after===null?{before:[],after:[]}:await tokenize(source,language);cache.set(language,tokens);if(current()&&(languageOverrides.get(root+':'+path)??detectLanguage(path))===language)applyTokens(tokens)}catch(error){const tokens={before:[],after:[],notice:`Source highlighting unavailable: ${String(error)}`};cache.set(language,tokens);if(current())applyTokens(tokens)}
}
function bind() {
  app.querySelectorAll<HTMLButtonElement>('[data-asset-mode]').forEach(button=>button.onclick=()=>{assetMode=button.dataset.assetMode as 'image'|'code';render()});
  document.getElementById('syntax-language')?.addEventListener('change',event=>{const path=tab==='history'?historyPath:selected?.path;if(!repo||!path)return;const value=(event.target as HTMLSelectElement).value,key=repo.root+':'+path;if(value==='auto')languageOverrides.delete(key);else languageOverrides.set(key,value);render()});
  document.getElementById('copy-selected-commit')?.addEventListener('click',async event=>{const button=event.currentTarget as HTMLButtonElement;try{await copyText(commit!.oid);button.title='Copied';button.setAttribute('aria-label','Commit hash copied');}catch(reason){error=String(reason);render()}});
  app.querySelectorAll<HTMLButtonElement>('[data-toggle-panel]').forEach(button=>button.onclick=()=>{togglePanel(layout,button.dataset.togglePanel as 'sidebar'|'log'|'diff');saveLayout();render();});
  document.getElementById('open')?.addEventListener('click', pickRepo);
  document.getElementById('welcome-open')?.addEventListener('click', pickRepo);
  document.getElementById('open-path')?.addEventListener('click', () => openRepo((document.getElementById('repo-path') as HTMLInputElement).value));
  document.getElementById('repo-path')?.addEventListener('keydown', e => { if ((e as KeyboardEvent).key === 'Enter') openRepo((e.target as HTMLInputElement).value); });
  document.getElementById('dismiss')?.addEventListener('click', () => { error = ''; render(); });
  document.getElementById('dismiss-note')?.addEventListener('click', () => { notification = ''; render(); });
  document.getElementById('view-mode')?.addEventListener('click', () => { split = !split; render(); });
  app.querySelectorAll<HTMLButtonElement>('[data-tab]').forEach(el => el.onclick = () => action(async () => { tab = el.dataset.tab as typeof tab; if (tab === 'history') { if (!historyLoaded) await loadHistory(); if (historyPath) await loadDiff(); else diff = null; } else await reconcileSelection(); render(); }));
  app.querySelectorAll<HTMLButtonElement>('[data-file]').forEach(el => el.onclick = () => action(async () => { selected = { path: repo!.files[Number(el.dataset.file)].path, staged: el.dataset.staged === 'true' }; await loadDiff(); }));
  app.querySelectorAll<HTMLButtonElement>('[data-stage]').forEach(el => el.onclick = () => action(async () => { const path = repo!.files[Number(el.dataset.stage)].path; repo = await api<Repo>({ op: 'stage_file', path, staged: el.dataset.staged === 'true' }); historyLoaded = false; await reconcileSelection(); }));
  app.querySelectorAll<HTMLButtonElement>('[data-hunk]').forEach(el => el.onclick = () => action(async () => { repo = await api<Repo>({ op: 'stage_hunk', path: selected!.path, staged: selected!.staged, patch: diff!.patch, hunk: Number(el.dataset.hunk) }); await reconcileSelection(); }));
  document.getElementById('message')?.addEventListener('input', e => { message = (e.target as HTMLTextAreaElement).value; if (repo) localStorage.setItem('branchline.draft:' + repo.root, message); const button = document.getElementById('commit') as HTMLButtonElement; button.disabled = busy || !message.trim() || !repo?.stagedToken; });
  document.getElementById('filter')?.addEventListener('input', e => { filter = (e.target as HTMLInputElement).value; render(); });
  document.getElementById('commit')?.addEventListener('click', doCommit);
  app.querySelectorAll<HTMLButtonElement>('[data-commit]').forEach(el => {
    el.onclick = e => {if(!busy)void selectCommit(Number(el.dataset.commit), e.shiftKey, e.metaKey || e.ctrlKey);};
    el.oncontextmenu = e => {e.preventDefault();void openCommitMenu(Number(el.dataset.commit),e.clientX,e.clientY)};
    el.onkeydown=e=>{if(e.key==='ContextMenu'||e.shiftKey&&e.key==='F10'){e.preventDefault();const r=el.getBoundingClientRect();void openCommitMenu(Number(el.dataset.commit),r.left+30,r.bottom)}};
  });
  app.querySelectorAll<HTMLButtonElement>('[data-history-file]').forEach(el => el.onclick = () => {void selectHistoryFile(commitFiles[Number(el.dataset.historyFile)]);});
  document.getElementById('load-more')?.addEventListener('click', () => action(async () => { commits.push(...await api<Commit[]>({ op: 'history', skip: commits.length, all: logAll })); }));
}
async function action(fn: () => Promise<void>) {
  if (busy) return; historyRequest++;historyPending=false;const hadDialog=!!dialog; updateMenu=false; busy = true; error = ''; notification = ''; recoveryRef = ''; render();
  try { await fn(); } catch (e) { error = String(e); } finally { busy = false; render(); if(dialog&&!hadDialog)focusModal(); }
}
async function pickRepo() {
  if (busy) return;
  if (!isTauri()) { error = 'Open the Tauri desktop app to access local repositories.'; render(); return; }
  try { const path = await open({ directory: true, multiple: false, title: 'Open Git repository' }); if (typeof path === 'string') await openRepo(path); } catch (e) { error = String(e); render(); }
}
async function openRepo(path: string) {
  if (!path.trim()) return;
  await action(async () => { const opened = await api<Repo>({ op: 'open_quick', path: path.trim() }); repo = opened; branches=branchCache.peek(repo.root)??[];branchMenuOpen=false;branchLoadError=''; selected = null; diff = null; commits = []; selectedOids.clear(); dialog=null; menu=false; commit = null; commitFiles = []; historyPath = ''; filter = ''; message = localStorage.getItem('branchline.draft:' + repo.root) ?? ''; tab = 'changes'; historyLoaded = false; if(persistRepository)localStorage.setItem('branchline.repo', repo.root); await reconcileSelection();void finishOpening(repo.root); });
}
async function loadDiff() {
  const id = ++sequence; loadingDiff = true; diff = null; const view = app.querySelector('.diff-content'); if (view) view.scrollTop = 0; render();
  try { const result = tab === 'history' && commit && historyPath ? await api<Diff>({ op: 'commit_diff', oid: commit.oid, path: historyPath }) : selected ? await api<Diff>({ op: 'diff', ...selected }) : null; if (id === sequence) diff = result; } finally { if (id === sequence) loadingDiff = false; }
}
async function reconcileSelection() {
  if (!repo) return;
  const stillExists = selected && repo.files.some(f => f.path === selected!.path && (selected!.staged ? f.index !== ' ' && f.index !== '?' : f.worktree !== ' ' || f.index === '?'));
  if (!stillExists) { const f = repo.files.find(f => f.worktree !== ' ' || f.index === '?') ?? repo.files[0]; selected = f ? { path: f.path, staged: f.worktree === ' ' && f.index !== '?' } : null; }
  if (selected) await loadDiff(); else diff = null;
}
async function loadHistory() { commits = await api<Commit[]>({ op: 'history', skip: 0, all: logAll }); historyLoaded = true; }
async function refresh() { if (!repo) return; repo = await api<Repo>({ op: 'status' });void prefetchBranches(true); if (tab === 'changes') await reconcileSelection(); else { await loadHistory(); selectedOids = new Set([...selectedOids].filter(oid=>commits.some(c=>c.oid===oid))); if (commit && !commits.some(c=>c.oid===commit!.oid)) {commit=null;commitFiles=[];historyPath='';diff=null;} if (historyPath) await loadDiff(); } }
async function doCommit() {
  if (!repo || !message.trim() || !repo.stagedToken) return;
  await action(async () => { repo = await api<Repo>({ op: 'commit', message, expected: repo!.stagedToken }); message = ''; localStorage.removeItem('branchline.draft:' + repo!.root); historyLoaded = false; notification = 'Commit created. Your staged changes are now in history.'; await reconcileSelection(); });
}
document.addEventListener('keydown', e => { if (dialog||ai.isOpen()) return; if (!(e.metaKey || e.ctrlKey)) return; if (e.key.toLowerCase() === 'n') {e.preventDefault();void invoke('new_window').catch(reason=>{error=String(reason);render()});} if (e.key.toLowerCase() === 'o') { e.preventDefault(); pickRepo(); } if (e.key.toLowerCase() === 'r') { e.preventDefault(); action(refresh); } if (e.key === 'Enter' && tab === 'changes') { e.preventDefault(); doCommit(); } });
let lastInteraction=0;
const canAutoRefresh=()=>!isPanelResizing()&&!!repo&&!busy&&!historyPending&&!dialog&&!ai.isOpen()&&!updateMenu&&!menu&&!document.hidden&&Date.now()-lastInteraction>400;
document.addEventListener('pointerdown',()=>{lastInteraction=Date.now();});
document.addEventListener('keydown',()=>{lastInteraction=Date.now();});
const checkLocalChanges=createAutoRefresh({current:()=>repo,allowed:canAutoRefresh,revision:()=>api({op:'revision'}),refresh:refreshInBackground});
const autoTimer=window.setInterval(()=>{void checkLocalChanges().catch(()=>{/* Retry transient read failures on the next tick. */});},1000);
let focusRefresh:ReturnType<typeof setTimeout>;
window.addEventListener('focus',()=>{clearTimeout(focusRefresh);focusRefresh=setTimeout(()=>{void checkLocalChanges().catch(()=>{});},450);});
window.addEventListener('pagehide',()=>{clearInterval(autoTimer);clearTimeout(focusRefresh);});
render();
if(isTauri()){
  void (async()=>{
    let firstWindow=true;
    try{const context=await invoke<{root:string|null;fresh:boolean}>('window_context');if(context){firstWindow=!context.fresh;persistRepository=!context.fresh;savedRepository=context.root??(context.fresh?'':savedRepository);restoringRepository=!!savedRepository;render();}}catch{}
    ai.start();
    let presented=false;const present=()=>{if(presented)return;presented=true;void invoke('frontend_ready').catch(()=>{})};
    requestAnimationFrame(present);window.setTimeout(present,80);
    if(savedRepository)await openRepo(savedRepository).finally(()=>{restoringRepository=false;render()});
    if(firstWindow)void skillSetup.show(true);
  })();
}else ai.start();



function showDialog(value: Dialog) { previousFocus=document.activeElement as HTMLElement; dialog=value; branchMenuOpen=false; menu=false; updateMenu=false; error=''; }
function closeDialog() { if(busy)return; dialog=null;branchMenuOpen=false;error='';render(); if(previousFocus?.isConnected)previousFocus.focus();else (document.getElementById('log-actions')??document.getElementById('branches'))?.focus(); }
async function prefetchBranches(force=false){
  if(!repo)return;const root=repo.root;
  if(!force&&branchCache.peek(root)!==undefined)return;
  branchLoadError='';if(force)branchCache.invalidate(root);
  const pending=branchCache.refresh(root);if(dialog?.type==='branches')render();
  try{const value=await pending;if(repo?.root!==root||branchCache.peek(root)!==value)return;branches=value;if(selectedBranchReference&&!branches.some(b=>b.reference===selectedBranchReference))branchMenuOpen=false;}
  catch{if(repo?.root===root)branchLoadError='Couldn’t reload branches. Your cached list is still available; try Reload.';}
  finally{if(repo?.root===root&&dialog?.type==='branches')render();}
}
function closeBranchMenu(restoreFocus=false){
  branchMenuOpen=false;document.querySelector('.branch-action-menu')?.remove();
  const index=branches.findIndex(b=>b.reference===selectedBranchReference),row=document.querySelector<HTMLElement>(`[data-select-branch="${index}"]`);row?.setAttribute('aria-expanded','false');if(restoreFocus)row?.focus({preventScroll:true});
}
function branchActions(){
  const source=branches.find(b=>b.reference===selectedBranchReference);
  if(!source||!branchMenuOpen)return '';
  const actions=[...(!source.current?[['switch','arrow-right',source.remote?'Check out locally':'Switch to branch']]:[]),...(!source.remote?[['push','arrow-up','Push…'],['rename','pencil','Rename…']]:[]),['create','plus','New branch from here…'],['copy','copy','Copy name'],...(!source.remote&&!source.current?[['delete','trash-2','Delete merged branch…']]:[])];
  return `<div class="branch-action-menu" role="menu" aria-label="Actions for ${esc(source.name)}"><div class="branch-menu-heading">${esc(source.name)}</div>${actions.map(([kind,glyph,label])=>`<button role="menuitem" data-branch-action="${kind}" ${busy||kind==='push'&&!repo!.remotes.length?'disabled':''}>${icon(glyph)}<span>${label}</span></button>`).join('')}</div>`;
}
function renderDialog() {
  const d=dialog!; let title='', content='', submit='';
  if(d.type==='appearance') {
    title='Appearance'; content=appearanceContent(); submit='<button id="reset-appearance">Reset defaults</button><button id="appearance-done" class="primary">Done</button>';

  } else if(d.type==='branches') {
    title='Branches';
    const filtered=branches.map((b,i)=>({b,i})).filter(({b})=>b.name.toLowerCase().includes(branchFilter.toLowerCase()));
    content=`<input id="branch-filter" value="${esc(branchFilter)}" placeholder="Search branches" aria-label="Search branches"><div class="branch-list">${filtered.length?['Local','Remote'].map(group=>{const matches=filtered.filter(({b})=>b.remote===(group==='Remote'));return matches.length?`<div class="group-heading">${group}</div>${matches.map(({b,i})=>`<button data-select-branch="${i}" aria-pressed="${selectedBranchReference===b.reference}" aria-haspopup="menu" aria-expanded="${branchMenuOpen&&selectedBranchReference===b.reference}" ${busy?'disabled':''}>${icon('git-branch')}<span>${esc(b.name)}<small>${b.current?'Current branch':b.remote?'Available on remote':b.upstream?esc(b.upstream):'Local branch'}</small></span>${b.current?icon('check'):icon('chevron-right')}</button>`).join('')}`:''}).join(''):`${branchCache.peek(repo!.root)===undefined&&branchCache.loading(repo!.root)&&!branchFilter?'<div class="branch-loading" role="status">Loading branches…</div>':`<div class="branch-empty">${icon('git-branch')}<h3>${branchLoadError?'Couldn’t load branches':branchFilter?'No matching branches':'No branches available'}</h3><p>${branchLoadError?'Try Reload to read the local branch list again.':branchFilter?`Create “${esc(branchFilter.trim())}” from ${esc(repo!.branch)}.`:'Create a branch to start your next change.'}</p></div>`}`}</div>${branchLoadError?`<p class="dialog-note" role="status">${esc(branchLoadError)}</p>`:''}${branchActions()}`;
    submit=`<button id="new-branch" class="primary">${!filtered.length&&branchFilter.trim()?`Create “${esc(branchFilter.trim())}”…`:'New branch…'}</button>`;
  } else if(d.type==='branch-action') {
    title=d.kind==='rename'?'Rename branch':d.kind==='delete'?'Delete local branch':'Push branch';
    content=`<div class="branch-action-source">${icon('git-branch')}<strong>${esc(d.source.name)}</strong></div>${d.kind==='rename'?`<label>New name<input id="branch-action-name" value="${esc(d.name)}" aria-label="New branch name"></label><p class="dialog-note">Renames this local branch. Remote branch names stay unchanged.</p>`:d.kind==='delete'?`<p>Delete this local branch? Git will refuse if it has unmerged commits or is open in a worktree.</p><p class="dialog-note">Its remote branch stays unchanged.</p>`:`<label>Remote<select id="branch-action-remote" aria-label="Push remote">${repo!.remotes.map(remote=>`<option ${remote===d.remote?'selected':''}>${esc(remote)}</option>`).join('')}</select></label><label>Destination branch<input id="branch-action-destination" value="${esc(d.destination)}" aria-label="Destination branch"></label><p class="dialog-note">Push the selected branch’s commits, even if another branch is checked out. Creates the destination if needed; history rewrites are rejected. Your current branch and upstream settings stay unchanged.</p>${['main','master'].includes(d.destination.toLowerCase())||d.destination!==d.source.name?`<div class="branch-push-warning">${icon('flag')} ${['main','master'].includes(d.destination.toLowerCase())?`Updates ${esc(d.remote)}/${esc(d.destination)} directly. `:''}${d.destination!==d.source.name?'Source and destination branch names differ.':''}</div>`:''}`}`;
    submit=`<button id="dialog-submit" class="primary" ${busy||(d.kind==='rename'&&!d.name.trim())||(d.kind==='push'&&(!d.remote||!d.destination.trim()))?'disabled':''}>${d.kind==='rename'?'Rename branch':d.kind==='delete'?'Delete merged branch':'Push branch'}</button>`;
  } else if(d.type==='network') {
    title=d.kind==='pull'?'Update branch':d.kind==='fetch'?'Fetch only':'Push commits';
    content=repo!.remotes.length?`<label>Remote<select id="remote" aria-label="Remote">${repo!.remotes.map(r=>`<option ${r===d.remote?'selected':''}>${esc(r)}</option>`).join('')}</select></label>${d.kind==='fetch'?'':`<label>Remote branch<input id="remote-branch" value="${esc(d.branch)}" aria-label="Remote branch"></label>`}${trackingStatus(d.remote,d.kind==='fetch'?undefined:d.branch)}<div class="operation-preview">${d.kind==='fetch'?`Fetch branches and tags from <strong>${esc(d.remote)}</strong>. Your working tree stays as it is.`:d.kind==='pull'?`Fetch the latest commits from <strong>${esc(d.remote)}/${esc(d.branch)}</strong>, then fast-forward <strong>${esc(repo!.branch)}</strong>. If history has diverged, your branch stays unchanged.`:`Push <strong>${esc(repo!.branch)}</strong> to <strong>${esc(d.remote)}/${esc(d.branch)}</strong> and track that branch. Non-fast-forward updates are rejected.`}</div><p class="dialog-note">Uses your configured Git credential helpers and SSH agent. If authentication needs a terminal prompt, sign in with Git first.</p>`:'<p>No remotes are configured. Add a remote with Git; it will appear automatically.</p>';
    submit=`<button id="dialog-submit" class="primary" ${!repo!.remotes.length||busy?'disabled':''}>${d.kind==='pull'?'Update':d.kind==='fetch'?'Fetch only':'Push'}</button>`;
  } else if(d.type==='rewrite') {
    title=d.kind==='squash'?`Squash ${d.oids.length} commits`:'Edit commit message';
    content=`<div class="operation-preview">${d.kind==='squash'?`${d.oids.length} consecutive commits will become one.`:'The selected commit will receive a new message.'} ${d.preview.plan.affected.length-d.oids.length} later commits will be replayed on <strong>${esc(repo!.branch)}</strong>.</div><ul class="rewrite-commits">${d.preview.plan.selected.map(oid=>`<li><code>${esc(oid.slice(0,8))}</code> ${esc(commits.find(c=>c.oid===oid)?.subject??'')}</li>`).join('')}</ul><label>Commit message<textarea id="rewrite-message" aria-label="New commit message">${esc(d.message)}</textarea></label><p class="dialog-note">Commit hashes will change. A local backup ref is saved before rewriting. Only history absent from fetched remote refs is eligible; fetch first to update that check.</p>`;
    submit=`<button id="dialog-submit" class="primary" ${busy||!d.message.trim()?'disabled':''}>${d.kind==='squash'?'Squash commits':'Save new message'}</button>`;
  } else if(d.type==='commit-action') {
    title=d.kind==='cherry-pick'?'Cherry-pick commit':'Revert commit';
    content=`<div class="operation-preview"><strong>${esc(commits.find(c=>c.oid===d.oid)?.subject??d.oid)}</strong><br><code>${esc(d.oid.slice(0,12))}</code><p>${d.kind==='cherry-pick'?'Apply this commit as a new commit':'Create a new commit reversing this change'} on <strong>${esc(repo!.branch)}</strong>.</p></div><p class="dialog-note">Requires a clean working tree. If Git encounters conflicts, the app attempts to abort and restore the original state.</p>`;
    submit=`<button id="dialog-submit" class="primary" ${busy?'disabled':''}>${d.kind==='cherry-pick'?'Cherry-pick':'Revert'}</button>`;
  } else if(d.type==='create') {
    const source=d.sourceName??(d.reference===repo!.head?repo!.branch:d.reference.startsWith('refs/')?d.reference.replace(/^refs\/(remotes|heads)\//,''):commits.find(c=>c.oid===d.reference)?.subject??'the selected commit');
    title='New branch';content=`<label>Branch name<input id="new-branch-name" value="${esc(d.name)}" aria-label="Branch name" placeholder="feature/my-change"></label><p class="dialog-note">Start from <strong>${esc(source)}</strong>. You’ll switch to your new branch.${d.reference.startsWith('refs/remotes/')?' It will follow updates from this remote branch.':''}</p><details class="branch-source"><summary>Starting point details</summary><code>${esc(d.reference)}</code></details>`;
    submit=`<button id="dialog-submit" class="primary" ${busy||!d.name.trim()?'disabled':''}>Create branch</button>`;
  } else { title=`Abort ${d.kind}`;content='<p>Abort the current Git operation and ask Git to restore its starting state. Changes made while resolving conflicts may be lost.</p>';submit='<button id="dialog-submit" class="primary">Abort operation</button>'; }
  return `<div class="modal-backdrop"><section class="modal" role="dialog" aria-modal="true" aria-labelledby="dialog-title"><header><h2 id="dialog-title">${title}</h2>${d.type==='branches'?`<button id="branch-refresh" class="icon-button" title="Reload local branches" aria-label="Reload local branches" ${branchCache.loading(repo!.root)?'disabled':''}>${icon('refresh-cw')}</button>`:''}<button id="dialog-close" aria-label="Close dialog" ${busy?'disabled':''}>${icon('x')}</button></header><div class="modal-content">${content}${error?`<div class="dialog-error" role="alert">${esc(error)}</div>`:''}</div><footer>${d.type!=='appearance'?`<button id="dialog-cancel" ${busy?'disabled':''}>Close</button>`:''}${submit}</footer></section></div>`;
}
function bindOperations() {
  bindAppearance();
  document.getElementById('update')?.addEventListener('click',()=>openNetwork('pull'));
  document.getElementById('update-options')?.addEventListener('click',()=>{updateMenu=!updateMenu;render();if(updateMenu)(document.querySelector('.update-menu button') as HTMLElement|null)?.focus();});
  document.getElementById('update-options')?.addEventListener('keydown',e=>{if(e.key==='ArrowDown'){e.preventDefault();updateMenu=true;render();(document.querySelector('.update-menu button') as HTMLElement|null)?.focus();}});
  document.getElementById('fetch-only')?.addEventListener('click',fetchOnly);
  document.getElementById('copy-recovery')?.addEventListener('click',async()=>{try{await copyText(recoveryRef); const el=document.getElementById('copy-recovery');if(el)el.textContent='Copied';}catch(e){error=String(e);render();}});
  document.getElementById('branches')?.addEventListener('click',()=>{if(busy||!repo)return;branches=branchCache.peek(repo.root)??[];branchFilter='';selectedBranchReference='';showDialog({type:'branches'});render();focusModal();void prefetchBranches();});
  document.getElementById('branch-refresh')?.addEventListener('click',()=>{closeBranchMenu();void prefetchBranches(true)});
  document.getElementById('branch-filter')?.addEventListener('input',e=>{branchFilter=(e.target as HTMLInputElement).value;selectedBranchReference='';branchMenuOpen=false;render();});
  app.querySelectorAll<HTMLButtonElement>('[data-select-branch]').forEach(button=>{
    const select=(point?:{x:number;y:number;alignRight:boolean})=>{const bounds=button.getBoundingClientRect();selectedBranchReference=branches[Number(button.dataset.selectBranch)].reference;branchMenuPoint=point??{x:bounds.right,y:bounds.top+Math.min(32,bounds.height),alignRight:true};branchMenuOpen=true;render();document.querySelector<HTMLElement>('.branch-action-menu button:not(:disabled)')?.focus({preventScroll:true})};
    button.onclick=()=>select();button.oncontextmenu=event=>{event.preventDefault();select({x:event.clientX,y:event.clientY,alignRight:false})};
    button.onkeydown=event=>{if(event.key==='ArrowRight'||event.key==='ContextMenu'||event.key==='F10'&&event.shiftKey){event.preventDefault();select()}};
  });
  app.querySelectorAll<HTMLButtonElement>('[data-branch-action]').forEach(button=>button.onclick=()=>void action(async()=>{
    const source=branches.find(b=>b.reference===selectedBranchReference);if(!source)return;
    const kind=button.dataset.branchAction!;branchMenuOpen=false;
    if(kind==='copy'){await copyText(source.name);return;}
    if(kind==='create'){showDialog({type:'create',reference:source.oid,sourceName:source.name,head:repo!.stateToken,name:''});return;}
    if(kind==='switch'){
      if(source.remote){const remote=repo!.remotes.slice().sort((a,b)=>b.length-a.length).find(r=>source.name.startsWith(r+'/'));showDialog({type:'create',reference:source.reference,sourceName:source.name,head:repo!.stateToken,name:remote?source.name.slice(remote.length+1):source.name});}
      else{await updateRepo({op:'switch_branch',reference:source.reference,create:null,expected_head:repo!.stateToken},`Checked out ${source.name}`);dialog=null;}
      return;
    }
    const upstreamRemote=repo!.remotes.slice().sort((a,b)=>b.length-a.length).find(remote=>source.upstream.startsWith(remote+'/'));
    showDialog({type:'branch-action',kind:kind as 'rename'|'delete'|'push',source:{...source},head:repo!.stateToken,name:kind==='rename'?source.name:'',remote:upstreamRemote??repo!.remotes[0]??'',destination:upstreamRemote?source.upstream.slice(upstreamRemote.length+1):source.name});
  }).then(()=>{if(dialog?.type==='branches')closeBranchMenu(true);else if(dialog)focusModal()}));
  for(const [id,key] of [['branch-action-name','name'],['branch-action-destination','destination'],['branch-action-remote','remote']] as const){document.getElementById(id)?.addEventListener(id==='branch-action-remote'?'change':'input',event=>{if(dialog?.type==='branch-action'){dialog[key]=(event.target as HTMLInputElement).value;render();}});}
  document.getElementById('new-branch')?.addEventListener('click',()=>{showDialog({type:'create',reference:repo!.head,head:repo!.stateToken,name:branchFilter.trim()});render();focusModal();});
  app.querySelectorAll<HTMLButtonElement>('[data-network]').forEach(el=>el.onclick=()=>{
    openNetwork(el.dataset.network!);
  });
  document.getElementById('remote')?.addEventListener('change',e=>{if(dialog?.type==='network'){dialog.remote=(e.target as HTMLSelectElement).value;render();}});
  document.getElementById('remote-branch')?.addEventListener('input',e=>{if(dialog?.type==='network'){dialog.branch=(e.target as HTMLInputElement).value;render();}});
  document.getElementById('rewrite-message')?.addEventListener('input',e=>{if(dialog?.type==='rewrite'){dialog.message=(e.target as HTMLTextAreaElement).value;(document.getElementById('dialog-submit') as HTMLButtonElement).disabled=busy||!dialog.message.trim();}});
  document.getElementById('new-branch-name')?.addEventListener('input',e=>{if(dialog?.type==='create'){dialog.name=(e.target as HTMLInputElement).value;(document.getElementById('dialog-submit') as HTMLButtonElement).disabled=busy||!dialog.name.trim();}});
  document.getElementById('dialog-submit')?.addEventListener('click',submitDialog);
  document.getElementById('dialog-close')?.addEventListener('click',closeDialog);
  document.getElementById('dialog-cancel')?.addEventListener('click',closeDialog);
  document.getElementById('abort-operation')?.addEventListener('click',()=>{showDialog({type:'abort',kind:repo!.operation!});render();focusModal();});
  document.getElementById('log-scope')?.addEventListener('change',e=>{const all=(e.target as HTMLSelectElement).value==='all';action(async()=>{logAll=all;selectedOids.clear();commit=null;historyPath='';commitFiles=[];diff=null;menu=false;await loadHistory();});});
  document.getElementById('log-filter')?.addEventListener('input',e=>{logFilter=(e.target as HTMLInputElement).value;render();});
  document.getElementById('log-actions')?.addEventListener('keydown',event=>{if(event.key==='ArrowDown'){event.preventDefault();(event.currentTarget as HTMLButtonElement).click();}});
  document.getElementById('log-actions')?.addEventListener('click',e=>{const r=(e.currentTarget as HTMLElement).getBoundingClientRect();menuPoint={x:r.left,y:r.bottom+4};menu=!menu;render();if(menu)(document.querySelector('.log-action-menu button:not(:disabled)') as HTMLElement|null)?.focus();});
  app.querySelectorAll<HTMLButtonElement>('[data-log-action]').forEach(el=>el.onclick=()=>openLogAction(el.dataset.logAction!));
}
async function selectCommit(index:number,range:boolean,toggle:boolean) {
  const c=commits[index];if(!c||!repo)return;const root=repo.root,id=++historyRequest;menu=false;historyPending=true;
  if(range){selectedOids=new Set(commits.slice(Math.min(anchor,index),Math.max(anchor,index)+1).map(c=>c.oid));}
  else if(toggle){if(selectedOids.has(c.oid))selectedOids.delete(c.oid);else selectedOids.add(c.oid);anchor=index;}
  else {selectedOids=new Set([c.oid]);anchor=index;}
  app.querySelectorAll<HTMLButtonElement>('[data-commit]').forEach(el=>{const on=selectedOids.has(commits[Number(el.dataset.commit)].oid);el.classList.toggle('selected',on);el.setAttribute('aria-pressed',String(on));});
  const actions=document.getElementById('log-actions') as HTMLButtonElement|null;if(actions)actions.disabled=true;
  try {
    const key=root+':'+c.oid;let next=historyCache.get(key);
    if(!next){const files=await api<string[]>({op:'commit_files',oid:c.oid});if(id!==historyRequest||repo?.root!==root||tab!=='history')return;
      const path=files.includes(historyPath)?historyPath:files[0]??'';
      const result=path?await api<Diff>({op:'commit_diff',oid:c.oid,path}):null;
      next={files,path,diff:result};if(historyCache.size>=40)historyCache.delete(historyCache.keys().next().value!);historyCache.set(key,next);
    }
    if(id!==historyRequest||repo?.root!==root||tab!=='history')return;
    commit=c;commitFiles=next.files;historyPath=next.path;diff=next.diff;loadingDiff=false;historyPending=false;error='';render();
    const content=app.querySelector('.diff-content');if(content)content.scrollTop=0;
  }catch(ex){if(id===historyRequest){historyPending=false;error=String(ex);render();}}
}
async function selectHistoryFile(path:string){
  if(!commit||!repo||busy)return;const c=commit,root=repo.root,id=++historyRequest;historyPending=true;
  try{const result=await api<Diff>({op:'commit_diff',oid:c.oid,path});if(id!==historyRequest||repo?.root!==root||tab!=='history')return;
    historyPath=path;diff=result;historyPending=false;historyCache.set(root+':'+c.oid,{files:commitFiles,path,diff:result});render();const view=app.querySelector('.diff-content');if(view)view.scrollTop=0;
  }catch(ex){if(id===historyRequest){historyPending=false;error=String(ex);render();}}
}
async function openCommitMenu(index:number,x:number,y:number){
  if(busy)return;const oid=commits[index]?.oid;if(!oid)return;
  if(!selectedOids.has(oid)||historyPending)await selectCommit(index,false,false);
  if(historyPending||!selectedOids.has(oid))return;
  menuPoint={x,y};menu=true;render();(document.querySelector('.log-action-menu button:not(:disabled)') as HTMLElement|null)?.focus();
}

async function openLogAction(kind:string) {
  const oids=[...selectedOids]; if(!oids.length||historyPending)return;
  await action(async()=>{
    menu=false;
    if(kind==='copy'){await copyText(oids[0]);notification='Commit hash copied.';return;}
    if(kind==='squash'||kind==='reword'){
      const preview=await api<RewritePreview>({op:'rewrite_preview',kind,oids});showDialog({type:'rewrite',kind,oids,preview,message:preview.plan.message});
    } else if(kind==='create')showDialog({type:'create',reference:oids[0],head:repo!.stateToken,name:''});
    else showDialog({type:'commit-action',kind,oid:oids[0],head:repo!.stateToken});
  });focusModal();
}
async function updateRepo(request:object,note:string) {
  repo=await api<Repo>(request);void prefetchBranches(true);historyLoaded=false;selectedOids.clear();menu=false;commit=null;commitFiles=[];historyPath='';diff=null;
  if(tab==='history')await loadHistory();else await reconcileSelection();
  notification=repo.operationResult??note; recoveryRef=repo.backupRef??'';
}
async function submitDialog() {
  const d=dialog;if(!d)return;
  await action(async()=>{
    if(d.type==='branch-action')await updateRepo({op:'branch_action',kind:d.kind,reference:d.source.reference,expected_oid:d.source.oid,name:d.name,remote:d.remote,branch:d.destination,expected_head:d.head},d.kind==='rename'?`Renamed ${d.source.name} to ${d.name}`:d.kind==='delete'?`Deleted ${d.source.name}`:`Pushed ${d.source.name}`);
    else if(d.type==='network')await updateRepo({op:'network',kind:d.kind,remote:d.remote,branch:d.branch,expected_head:d.head},d.kind==='pull'?'Branch updated.':d.kind==='fetch'?'Remote checked. Your files are unchanged.':'Commits pushed.');
    else if(d.type==='rewrite')await updateRepo({op:'rewrite',kind:d.kind,oids:d.oids,message:d.message,expected:d.preview.token},'History updated.');
    else if(d.type==='commit-action')await updateRepo({op:'commit_action',kind:d.kind,oid:d.oid,expected_head:d.head},'Commit action completed.');
    else if(d.type==='create')await updateRepo({op:'switch_branch',reference:d.reference,create:d.name,expected_head:d.head},`Checked out ${d.name}`);
    else if(d.type==='abort')await updateRepo({op:'abort_operation',kind:d.kind},'Operation aborted.');
    dialog=null;
  });
  if(error&&repo){try{repo=await api<Repo>({op:'status'});render();}catch{/* Keep the original operation error. */}}
}
function focusModal() { (document.querySelector('.modal input,.modal textarea,.modal select,.modal button') as HTMLElement|null)?.focus(); }
document.addEventListener('keydown',e=>{
  if(!dialog){if(e.key==='Escape'&&(menu||updateMenu)){const target=updateMenu?'update-options':'log-actions';menu=false;updateMenu=false;render();document.getElementById(target)?.focus();}return;}
  if(branchMenuOpen){
    if(e.key==='Escape'){e.preventDefault();e.stopImmediatePropagation();closeBranchMenu(true);return;}
    if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){e.preventDefault();const items=[...document.querySelectorAll<HTMLButtonElement>('.branch-action-menu button:not(:disabled)')];let index=items.indexOf(document.activeElement as HTMLButtonElement);index=e.key==='Home'?0:e.key==='End'?items.length-1:(index+(e.key==='ArrowDown'?1:-1)+items.length)%items.length;items[index]?.focus();return;}
    if(e.key==='Tab')closeBranchMenu(true);
  }
  if(dialog.type==='branches'&&(e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==='r'){e.preventDefault();closeBranchMenu();void prefetchBranches(true);return;}
  if(e.key==='Escape'){e.preventDefault();closeDialog();}
  if(e.key==='Tab'){
    const focusable=[...document.querySelectorAll<HTMLElement>('.modal button:not(:disabled),.modal input,.modal textarea,.modal select')];
    const first=focusable[0],last=focusable.at(-1);
    if(e.shiftKey&&document.activeElement===first){e.preventDefault();last?.focus();}
    else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first?.focus();}
  }
},true);

function appearanceContent() {
  return `<p class="settings-intro">Changes preview live and save automatically on this Mac.</p>
  <div class="setting"><label for="ui-font-size"><span>Interface text</span><output id="ui-font-value">${appearance.fontSize} px</output></label><input id="ui-font-size" type="range" min="12" max="18" step="1" value="${appearance.fontSize}"><div class="range-labels"><span>Smaller</span><span>Larger</span></div></div>
  <div class="setting"><label for="code-font-size"><span>Diff text</span><output id="code-font-value">${appearance.codeSize} px</output></label><input id="code-font-size" type="range" min="12" max="20" step="1" value="${appearance.codeSize}"><div class="range-labels"><span>Smaller</span><span>Larger</span></div><code class="appearance-code-preview">const branch = "main";</code></div>
  <fieldset class="setting spacing-setting"><legend>Spacing</legend><div class="spacing-options">${(['compact','comfortable','relaxed'] as const).map(value=>`<label><input type="radio" name="spacing" value="${value}" ${appearance.spacing===value?'checked':''}><span>${value[0].toUpperCase()+value.slice(1)}</span></label>`).join('')}</div></fieldset>
  <div class="setting"><label for="dark-contrast"><span>Dark-mode contrast</span><output id="contrast-value">${appearance.contrast}%</output></label><input id="dark-contrast" type="range" min="0" max="100" step="5" value="${appearance.contrast}"><div class="range-labels"><span>Soft charcoal</span><span>Deep black</span></div></div>
  <label class="simple-setting"><input id="simplified-ui" type="checkbox" ${appearance.simplified?'checked':''}><span><strong>Simplified interface</strong><small>Fewer controls and less metadata. Commit details stay available when you select a row.</small></span></label>`;
}
function saveAppearance() {
  appearance=parseAppearance(JSON.stringify(appearance));
  applyAppearance(appearance);
  localStorage.setItem(appearanceKey,JSON.stringify(appearance));
}
function bindAppearance() {
  document.getElementById('appearance')?.addEventListener('click',()=>{if(busy)return;showDialog({type:'appearance'});render();focusModal();});
  const range=(id:string,key:'fontSize'|'codeSize'|'contrast',output:string,suffix:string)=>document.getElementById(id)?.addEventListener('input',e=>{
    appearance[key]=Number((e.target as HTMLInputElement).value);saveAppearance();
    const result=document.getElementById(output);if(result)result.textContent=appearance[key]+suffix;
  });
  range('ui-font-size','fontSize','ui-font-value',' px');range('code-font-size','codeSize','code-font-value',' px');range('dark-contrast','contrast','contrast-value','%');
  app.querySelectorAll<HTMLInputElement>('input[name="spacing"]').forEach(el=>el.addEventListener('change',()=>{appearance.spacing=el.value as Appearance['spacing'];saveAppearance();}));
  document.getElementById('simplified-ui')?.addEventListener('change',e=>{appearance.simplified=(e.target as HTMLInputElement).checked;saveAppearance();});
  document.getElementById('reset-appearance')?.addEventListener('click',()=>{appearance={...defaultAppearance};saveAppearance();render();document.getElementById('reset-appearance')?.focus();});
  document.getElementById('appearance-done')?.addEventListener('click',closeDialog);
}

function preferredRemote() { return repo!.remotes.includes(repo!.tracking.remote??'')?repo!.tracking.remote!:repo!.remotes[0]??''; }
function openNetwork(kind:string) {
  if(busy)return;
  showDialog({type:'network',kind,remote:preferredRemote(),branch:repo!.tracking.merge??repo!.branch,head:repo!.stateToken});render();focusModal();
}
function trackingStatus(remote?:string,branch?:string) {
  const target=branch&&remote?`${remote}/${branch}`:repo!.tracking.upstream;
  const known=!!target&&target===repo!.tracking.upstream;
  return `<div class="tracking-visual" aria-label="Remote branch status"><span class="remote-badge" title="${esc(target??remote??'No tracking branch')}">${icon('git-branch')}<span class="remote-name">${esc(target??remote??'No tracking branch')}</span></span>${known?`<span class="transfer-indicator outgoing ${repo!.tracking.ahead?'has-count':''}" title="${repo!.tracking.ahead} local commits to push" aria-label="${repo!.tracking.ahead} outgoing commits">${icon('arrow-up')}<strong>${repo!.tracking.ahead}</strong><span>outgoing</span></span><span class="transfer-indicator incoming ${repo!.tracking.behind?'has-count':''}" title="${repo!.tracking.behind} remote commits to bring into this branch" aria-label="${repo!.tracking.behind} incoming commits">${icon('arrow-down')}<strong>${repo!.tracking.behind}</strong><span>incoming</span></span>`:''}</div>`;
}
function updateOptions() {
  return `<div id="update-menu" class="update-menu" role="menu" aria-label="Update options">${trackingStatus()}<button id="fetch-only" role="menuitem">${icon('refresh-cw')}<span><strong>Fetch only</strong><small>Check for updates without pulling.</small></span></button></div>`;
}
async function fetchOnly() {
  const remote=preferredRemote();
  if(!remote||(!repo!.tracking.remote&&repo!.remotes.length>1)){openNetwork('fetch');return;}
  const expected=repo!.stateToken;
  await action(async()=>{repo=await api<Repo>({op:'network',kind:'fetch',remote,branch:'',expected_head:expected});historyLoaded=false;if(tab==='history')await loadHistory();notification='Remote checked. Your files are unchanged.';});
  document.getElementById('update-options')?.focus();
}
document.addEventListener('click',e=>{
  if(updateMenu&&!(e.target as Element).closest('.update-control')){updateMenu=false;render();}
});

async function refreshInBackground() {
  if(!canAutoRefresh()||!repo)return;
  const root=repo.root, view=tab, priorSelection=JSON.stringify(selected), priorCommit=commit?.oid, priorPath=historyPath, priorScope=logAll;
  const current=await api<Repo>({op:'status'});
  const valid=()=>canAutoRefresh()&&repo?.root===root&&tab===view&&JSON.stringify(selected)===priorSelection&&commit?.oid===priorCommit&&historyPath===priorPath&&logAll===priorScope;
  if(!valid())return;
  let nextSelected=selected, nextDiff=diff, nextCommits=commits;
  if(view==='changes'){
    const exists=nextSelected&&current.files.some(f=>f.path===nextSelected!.path&&(nextSelected!.staged?f.index!==' '&&f.index!=='?':f.worktree!==' '||f.index==='?'));
    if(!exists){const f=current.files.find(f=>f.worktree!==' '||f.index==='?')??current.files[0];nextSelected=f?{path:f.path,staged:f.worktree===' '&&f.index!=='?'}:null;}
    nextDiff=nextSelected?await api<Diff>({op:'diff',...nextSelected}):null;
  }else{
    const firstPage=await api<Commit[]>({op:'history',skip:0,all:logAll});
    nextCommits=JSON.stringify(firstPage)===JSON.stringify(commits.slice(0,firstPage.length))?commits:firstPage;
    // A selected immutable commit remains reviewable even if it moves off the first page.
  }
  if(!valid())return;
  const key=(r:Repo)=>JSON.stringify([r.head,r.branchRef,r.stagedToken,r.files,r.lineStats,r.tracking,r.operation,r.remotes]);
  const changed=key(repo!)!==key(current)||JSON.stringify(diff)!==JSON.stringify(nextDiff)||JSON.stringify(commits)!==JSON.stringify(nextCommits)||JSON.stringify(selected)!==JSON.stringify(nextSelected);
  repo=current;void prefetchBranches(true);selected=nextSelected;diff=nextDiff;commits=nextCommits;
  if(changed)render();
}

// Context menus follow the pointer and dismiss without moving the selected commit.
document.addEventListener('pointerdown',ev=>{if(menu&&!(ev.target as Element).closest('.log-action-menu,#log-actions')){menu=false;document.querySelector('.log-action-menu')?.remove();document.getElementById('log-actions')?.setAttribute('aria-expanded','false')}});
window.addEventListener('resize',()=>{if(menu){menu=false;render()}});
document.addEventListener('keydown',ev=>{if(!menu||!['ArrowDown','ArrowUp','Home','End'].includes(ev.key))return;ev.preventDefault();const items=[...document.querySelectorAll<HTMLButtonElement>('.log-action-menu button:not(:disabled)')];let i=items.indexOf(document.activeElement as HTMLButtonElement);i=ev.key==='Home'?0:ev.key==='End'?items.length-1:(i+(ev.key==='ArrowDown'?1:-1)+items.length)%items.length;items[i]?.focus()});

document.addEventListener('pointerdown',event=>{if(branchMenuOpen&&!(event.target as Element).closest('.branch-action-menu,[data-select-branch]'))closeBranchMenu()});
document.addEventListener('wheel',event=>{if(branchMenuOpen&&(event.target as Element).closest('.branch-list'))closeBranchMenu()},{passive:true});
window.addEventListener('resize',()=>{if(branchMenuOpen)closeBranchMenu(true)});
