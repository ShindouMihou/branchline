import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,existsSync,rmSync,symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {createHash} from 'node:crypto';
import {cleanupReleaseCaches} from '../scripts/release-cleanup.mjs';

function fixture(){
 const root=mkdtempSync(join(tmpdir(),'branchline-cleanup-test-'));
 const write=(name,text)=>{const file=join(root,name);mkdirSync(dirname(file),{recursive:true});writeFileSync(file,text);return file};
 const version='0.5.0',releaseDirectory=join(root,'.local/releases/v0.5.0');
 write('package.json',JSON.stringify({name:'branchline',version}));
 write('src-tauri/tauri.conf.json',JSON.stringify({version}));
 write('src-tauri/target/release/bundle/macos/Branchline.app/Contents/MacOS/branchline','retained app');
 write('.local/releases/v0.5.0/Branchline.app.tar.gz','retained archive');
 write('.local/releases/v0.5.0/Branchline.app.tar.gz.sig','retained signature');
 write('.local/releases/v0.5.0/Branchline-macOS-arm64.zip','retained ZIP');
 write('.local/releases/v0.5.0/Branchline-macOS-arm64.zip.sha256',`${createHash('sha256').update('retained ZIP').digest('hex')}  Branchline-macOS-arm64.zip\n`);
 write('.local/releases/v0.5.0/latest.json',JSON.stringify({version,platforms:{'darwin-aarch64':{signature:'retained signature',url:'https://github.com/ShindouMihou/branchline/releases/download/v0.5.0/Branchline.app.tar.gz'}}}));
 for(const path of ['debug','release/build','release/deps','release/.fingerprint','release/incremental'])write(`src-tauri/target/${path}/generated`,'cache');
 write('src/precious.ts','source');write('.local/toolchain/cargo/keep','toolchain');
 write('.local/releases/v0.4.0/keep','older release');
 return {root,releaseDirectory,version,write,buildsRunning:()=>false};
}
function run(check){const context=fixture();try{check(context)}finally{rmSync(context.root,{recursive:true,force:true})}}

test('cleanup removes only generated caches and retains the app, releases, toolchain and source',()=>run(context=>{
 const result=cleanupReleaseCaches(context);assert.equal(result.removed.length,5);
 for(const path of ['src/precious.ts','.local/toolchain/cargo/keep','.local/releases/v0.4.0/keep','src-tauri/target/release/bundle/macos/Branchline.app/Contents/MacOS/branchline','.local/releases/v0.5.0/Branchline.app.tar.gz'])assert(existsSync(join(context.root,path)),path);
 assert.equal(cleanupReleaseCaches(context).removed.length,0);
}));
test('missing or mismatched release assets stop cleanup before anything is removed',()=>{
 for(const name of ['latest.json','Branchline.app.tar.gz','Branchline.app.tar.gz.sig','Branchline-macOS-arm64.zip','Branchline-macOS-arm64.zip.sha256'])run(context=>{
  rmSync(join(context.releaseDirectory,name));assert.throws(()=>cleanupReleaseCaches(context));assert(existsSync(join(context.root,'src-tauri/target/debug/generated')));
 });
 run(context=>{context.write('.local/releases/v0.5.0/Branchline-macOS-arm64.zip','modified');assert.throws(()=>cleanupReleaseCaches(context),/checksum/);assert(existsSync(join(context.root,'src-tauri/target/debug/generated')))});
});
test('active Rust builds preserve every cache',()=>run(context=>{
 assert.throws(()=>cleanupReleaseCaches({...context,buildsRunning:()=>true}),/active/);
 assert(existsSync(join(context.root,'src-tauri/target/debug/generated')));
}));
test('linked cache directories, including dangling links, are refused without traversing them',()=>{
 for(const dangling of [false,true])run(context=>{
  const cache=join(context.root,'src-tauri/target/debug');rmSync(cache,{recursive:true});
  const destination=join(context.root,dangling?'missing':'src');symlinkSync(destination,cache,'dir');
  assert.throws(()=>cleanupReleaseCaches(context),/symbolic links/);assert.equal(readFileSync(join(context.root,'src/precious.ts'),'utf8'),'source');
  assert(existsSync(join(context.root,'src-tauri/target/release/deps/generated')));
 });
});
test('a release directory placed inside a cache is protected',()=>run(context=>{
 const unsafe=join(context.root,'src-tauri/target/debug/release-assets');mkdirSync(unsafe);
 for(const name of ['latest.json','Branchline.app.tar.gz','Branchline.app.tar.gz.sig','Branchline-macOS-arm64.zip','Branchline-macOS-arm64.zip.sha256'])writeFileSync(join(unsafe,name),readFileSync(join(context.releaseDirectory,name)));
 assert.throws(()=>cleanupReleaseCaches({...context,releaseDirectory:unsafe}),/overlaps/);
 assert(existsSync(join(unsafe,'latest.json')));
}));
test('version mismatches and missing retained app preserve caches',()=>{
 run(context=>{assert.throws(()=>cleanupReleaseCaches({...context,version:'0.5.1'}),/versions/);assert(existsSync(join(context.root,'src-tauri/target/debug/generated')))});
 run(context=>{rmSync(join(context.root,'src-tauri/target/release/bundle/macos/Branchline.app'),{recursive:true});assert.throws(()=>cleanupReleaseCaches(context));assert(existsSync(join(context.root,'src-tauri/target/debug/generated')))});
});
