import assert from 'node:assert/strict';
import {chmodSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';

const binary=process.argv[2]??resolve(dirname(fileURLToPath(import.meta.url)),'../src-tauri/target/release/bundle/macos/Branchline.app/Contents/MacOS/branchline');
const root=mkdtempSync(join(tmpdir(),'branchline-wire-cycles-'));
const credential=join(root,'credential.json');
writeFileSync(credential,JSON.stringify({socket:join(root,'never-connect.sock'),clientId:'isolated-wire-test',token:'disposable-wire-token'}));
chmodSync(credential,0o600);
const rpc=(id,method,params={})=>({jsonrpc:'2.0',id,method,params});
const init=version=>rpc('init','initialize',{protocolVersion:version??'2025-11-25'});
const ready={jsonrpc:'2.0',method:'notifications/initialized'};
function run(messages,file=credential){
 const input=Buffer.isBuffer(messages)?messages:messages.map(message=>typeof message==='string'?message:JSON.stringify(message)).join('\n')+'\n';
 const result=spawnSync(binary,['--mcp','--credential-file',file],{input,encoding:'utf8',timeout:10000,maxBuffer:8*1024*1024});
 assert.ifError(result.error);
 const replies=result.stdout.trim()?result.stdout.trim().split('\n').map(line=>JSON.parse(line)):[];
 return {...result,replies};
}
function ok(messages){const result=run(messages);assert.equal(result.status,0,result.stderr);return result.replies;}
function cycle(number,name,check){check();console.log(`Cycle ${number}: ${name} PASS`);}
try {
 cycle(1,'supported protocol versions and public tool surface',()=>{
  for(const version of ['2025-03-26','2025-06-18','2025-11-25']){
   const replies=ok([init(version),ready,rpc('list','tools/list')]);
   assert.equal(replies[0].result.protocolVersion,version);
   const names=replies[1].result.tools.map(tool=>tool.name);
   assert(names.includes('branchline_propose_command'));assert(names.includes('branchline_wait_for_result'));
   assert(!names.some(name=>/approve|execute|propose_plan/.test(name)));
  }
 });
 cycle(2,'initialization guards and out-of-order notifications',()=>{
  const replies=ok([ready,rpc(1,'tools/list'),init(),rpc(2,'tools/list'),ready,rpc(3,'tools/list')]);
  assert.equal(replies[0].error.code,-32000);assert.equal(replies[2].error.code,-32000);assert(replies[3].result.tools.length>0);
 });
 cycle(3,'malformed JSON and unsupported JSON roots recover safely',()=>{
  const replies=ok(['not-json','{"jsonrpc":','[]','null','true',rpc('recovered','ping')]);
  assert.equal(replies.length,6);assert.equal(replies[0].error.code,-32700);assert.equal(replies[1].error.code,-32700);
  for(const reply of replies.slice(2,5))assert.equal(reply.error.code,-32600);
  assert.equal(replies[5].id,'recovered');assert.deepEqual(replies[5].result,{});
 });
 cycle(4,'Unicode and numeric response identity with unknown methods',()=>{
  const replies=ok([init(),ready,rpc('review → 日本語','ping'),rpc(42,'ping'),rpc('unknown','unknown/method')]);
  assert.equal(replies[1].id,'review → 日本語');assert.equal(replies[2].id,42);assert.equal(replies[3].error.code,-32601);
 });
 cycle(5,'duplicate initialization and repeated notifications',()=>{
  const replies=ok([init(),ready,ready,rpc(1,'initialize'),{jsonrpc:'2.0',method:'ignored/notification'},rpc(2,'ping')]);
  assert.equal(replies.length,3);assert.equal(replies[1].error.code,-32600);assert.deepEqual(replies[2].result,{});
 });
 cycle(6,'version negotiation and incompatible per-call metadata',()=>{
  const replies=ok([init('future-version'),ready,rpc(1,'tools/list',{_meta:{'io.modelcontextprotocol/protocolVersion':'future-version'}}),rpc(2,'ping')]);
  assert(['2025-03-26','2025-06-18','2025-11-25'].includes(replies[0].result.protocolVersion));assert.equal(replies[1].error.code,-32022);assert.deepEqual(replies[2].result,{});
 });
 cycle(7,'UTF-8 byte boundary and over-limit rejection',()=>{
  const base=JSON.stringify(rpc('limit','ping',{padding:'日本語'}));
  const padding=1048576-Buffer.byteLength(base)-1;
  const exact=Buffer.from(base+' '.repeat(padding)+'\n');assert.equal(exact.length,1048576);
  const accepted=run(exact);assert.equal(accepted.status,0);assert.equal(accepted.replies[0].id,'limit');
  const rejected=run(Buffer.concat([exact.subarray(0,-1),Buffer.from(' \n')]));assert.notEqual(rejected.status,0);assert.match(rejected.stderr,/exceeds 1 MiB/);
 });
 cycle(8,'credential permissions, symlinks and corrupt files',()=>{
  const link=join(root,'credential-link.json');symlinkSync(credential,link);
  const corrupt=join(root,'corrupt.json');writeFileSync(corrupt,'not-json',{mode:0o600});
  const directory=join(root,'directory');mkdirSync(directory,{mode:0o700});
  for(const file of [link,corrupt,directory])assert.notEqual(run([],file).status,0);
  chmodSync(credential,0o644);const unsafe=run([]);assert.notEqual(unsafe.status,0);assert.match(unsafe.stderr,/private regular file/);
 });
} finally {rmSync(root,{recursive:true,force:true});}
