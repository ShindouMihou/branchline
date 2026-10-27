import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,existsSync,rmSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {spawnSync} from 'node:child_process';

const installer=resolve('scripts/install.sh');
function fixture(t,options={}){
 const root=mkdtempSync(join(tmpdir(),'branchline-installer-test-'));
 t.after(()=>rmSync(root,{recursive:true,force:true}));
 const bin=join(root,'bin'),destination=join(root,'Applications'),calls=join(root,'calls');mkdirSync(bin);mkdirSync(destination);
 writeFileSync(join(root,'archive'),'controlled offline archive\n');
 const mock=`import fs from 'node:fs';import path from 'node:path';import {spawnSync} from 'node:child_process';import {createHash} from 'node:crypto';
const args=process.argv.slice(2),root=process.env.FIXTURE_ROOT;fs.appendFileSync(path.join(root,'calls'),JSON.stringify({tool,args})+'\\n');
if(tool==='uname')console.log(args[0]==='-s'?(process.env.OS_NAME||'Darwin'):(process.env.ARCH||'arm64'));
else if(tool==='id')console.log('501');
else if(tool==='sw_vers')console.log('14.0');
else if(tool==='sysctl')console.log(process.env.HW_ARM||'0');
else if(tool==='pgrep')process.exit(process.env.RUNNING==='1'?0:1);
else if(tool==='curl'){
 const out=args[args.indexOf('-o')+1],url=args.find(arg=>arg.startsWith('https://'));
 if(process.env.DOWNLOAD_FAIL==='1')process.exit(22);
 if(url.endsWith('/latest'))fs.writeFileSync(out,'{"tag_name":"v0.4.0"}');
 else if(url.endsWith('.sha256'))fs.writeFileSync(out,(process.env.BAD_CHECKSUM==='1'?'0'.repeat(64):createHash('sha256').update(fs.readFileSync(path.join(root,'archive'))).digest('hex'))+'  Branchline-macOS-arm64.zip\\n');
 else fs.copyFileSync(path.join(root,'archive'),out);
}else if(tool==='plutil')console.log(args.includes('tag_name')?(process.env.RELEASE_TAG||'v0.4.0'):(process.env.BUNDLE_ID||'dev.branchline.git'));
else if(tool==='codesign')process.exit(process.env.BAD_SIGNATURE==='1'?1:0);
else if(tool==='ditto'){
 if(args.includes('-x')){const app=path.join(args.at(-1),'Branchline.app');fs.mkdirSync(path.join(app,'Contents'),{recursive:true});fs.writeFileSync(path.join(app,'Contents','Info.plist'),'fixture');fs.writeFileSync(path.join(app,'new'),'new release');}
 else fs.cpSync(args[0],args[1],{recursive:true});
}else if(tool==='mv'){
 if(process.env.FAIL_REPLACE==='1'&&args[0].includes('.branchline-install.')&&args[0].endsWith('/Branchline.app'))process.exit(74);
 process.exit(spawnSync('/bin/mv',args,{stdio:'inherit'}).status);
}`;
 for(const tool of ['uname','id','sw_vers','sysctl','pgrep','curl','plutil','codesign','ditto','mv'])writeFileSync(join(bin,tool),'#!/usr/bin/env node\nconst tool='+JSON.stringify(tool)+';\n'+mock,{mode:0o755});
 const env={...process.env,PATH:bin+':'+process.env.PATH,FIXTURE_ROOT:root,BRANCHLINE_INSTALL_DIR:destination,...options};
 delete env.BRANCHLINE_VERSION;
 const run=()=>spawnSync('/bin/bash',[installer],{env,encoding:'utf8',timeout:15000});
 return {root,destination,app:join(destination,'Branchline.app'),run,calls:()=>existsSync(calls)?readFileSync(calls,'utf8').trim().split('\n').map(JSON.parse):[]};
}
test('help does not download or install',()=>{
 const result=spawnSync('/bin/bash',[installer,'--help'],{encoding:'utf8'});assert.equal(result.status,0);assert.match(result.stdout,/sudo/);
});
test('installs a verified release and cleans staging files',t=>{
 const f=fixture(t),result=f.run();assert.equal(result.status,0,result.stderr);assert.equal(readFileSync(join(f.app,'new'),'utf8'),'new release');
 const calls=f.calls();assert(calls.some(call=>call.tool==='codesign'&&call.args.includes('--strict')));assert(calls.some(call=>call.tool==='curl'&&call.args.includes('--proto')));
 assert(calls.some(call=>call.tool==='curl'&&call.args.some(arg=>arg.endsWith('/v0.4.0/Branchline-macOS-arm64.zip'))));
 assert(!calls.some(call=>['sudo','xattr','spctl','open'].includes(call.tool)));
});
test('updates an existing app after validation',t=>{
 const f=fixture(t);mkdirSync(f.app);writeFileSync(join(f.app,'old'),'previous');const result=f.run();assert.equal(result.status,0,result.stderr);assert(existsSync(join(f.app,'new')));assert(!existsSync(join(f.app,'old')));
});
test('checksum mismatch leaves the existing app untouched',t=>{
 const f=fixture(t,{BAD_CHECKSUM:'1'});mkdirSync(f.app);writeFileSync(join(f.app,'old'),'previous');const result=f.run();assert.notEqual(result.status,0);assert.match(result.stderr,/Checksum mismatch/);assert(existsSync(join(f.app,'old')));assert(!f.calls().some(call=>call.tool==='ditto'));
});
test('signature failure refuses installation',t=>{
 const f=fixture(t,{BAD_SIGNATURE:'1'}),result=f.run();assert.notEqual(result.status,0);assert.match(result.stderr,/signature did not verify/);assert(!existsSync(f.app));
});
test('unsupported platforms and Intel Macs stop before downloading',t=>{
 for(const options of [{OS_NAME:'Linux'},{ARCH:'x86_64',HW_ARM:'0'}]){const f=fixture(t,options),result=f.run();assert.notEqual(result.status,0);assert(!f.calls().some(call=>call.tool==='curl'));}
});
test('a running GUI app is not replaced',t=>{
 const f=fixture(t,{RUNNING:'1'}),result=f.run();assert.notEqual(result.status,0);assert.match(result.stderr,/Quit the Branchline app/);assert(!f.calls().some(call=>call.tool==='curl'));
});
test('invalid release tags cannot alter download paths',t=>{
 const f=fixture(t,{RELEASE_TAG:'../../different'}),result=f.run();assert.notEqual(result.status,0);assert.match(result.stderr,/release tag is invalid/);assert.equal(f.calls().filter(call=>call.tool==='curl').length,1);
});
test('failed final replacement restores the previous app',t=>{
 const f=fixture(t,{FAIL_REPLACE:'1'});mkdirSync(f.app);writeFileSync(join(f.app,'old'),'previous');const result=f.run();assert.notEqual(result.status,0);assert.equal(readFileSync(join(f.app,'old'),'utf8'),'previous');assert(!existsSync(join(f.app,'new')));
});

test('an unexpected bundle identity refuses installation',t=>{
 const f=fixture(t,{BUNDLE_ID:'other.application'}),result=f.run();assert.notEqual(result.status,0);assert.match(result.stderr,/unexpected bundle identity/);assert(!existsSync(f.app));
});
test('a failed download cannot replace an existing app',t=>{
 const f=fixture(t,{DOWNLOAD_FAIL:'1'});mkdirSync(f.app);writeFileSync(join(f.app,'old'),'previous');const result=f.run();assert.notEqual(result.status,0);assert(existsSync(join(f.app,'old')));assert(!f.calls().some(call=>call.tool==='ditto'));
});
