import {spawnSync} from 'node:child_process';
import {readFileSync,mkdirSync,copyFileSync,existsSync,writeFileSync,statSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {homedir} from 'node:os';
import {createHash} from 'node:crypto';
import {releaseManifest} from './release-manifest.mjs';
import {cleanupReleaseCaches} from './release-cleanup.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const run=(program,args,options={})=>{
 const result=spawnSync(program,args,{cwd:root,encoding:'utf8',stdio:'inherit',...options});
 if(result.error||result.status!==0)throw new Error(`${program} failed. Release preparation stopped.`);
 return result.stdout?.trim();
};
if(process.platform!=='darwin'||process.arch!=='arm64')throw new Error('Prepare releases on an Apple Silicon Mac.');
const key=process.env.TAURI_SIGNING_PRIVATE_KEY??join(homedir(),'.tauri/branchline-updater.key');
if(!existsSync(key)||!existsSync(`${key}.pub`))throw new Error('Provide a signing key file and its .pub file. See docs/UPDATES.md.');
if((statSync(key).mode&0o077)!==0)throw new Error('The signing key must be private. Set its file permissions to 600.');
const config=JSON.parse(readFileSync(join(root,'src-tauri/tauri.conf.json'),'utf8'));
if(readFileSync(`${key}.pub`,'utf8').trim()!==config.plugins.updater.pubkey)throw new Error('The signing key does not match the public key trusted by the app.');
const {version}=JSON.parse(readFileSync(join(root,'package.json'),'utf8'));
if(config.version!==version)throw new Error('Package and app versions must match.');
const notes=readFileSync(join(root,`docs/releases/v${version}.md`),'utf8');
const args=process.argv.slice(2);
if(args.length&&!(args.length===2&&args[0]==='--output'))throw new Error('Usage: npm run release:prepare -- [--output directory]');
const destination=args.length?resolve(root,args[1]):join(root,'.local/releases',`v${version}`);
if(existsSync(destination))throw new Error('Release output already exists. Bump the version or move the previous output aside.');
run(process.execPath,['scripts/tauri.mjs','build','--config',JSON.stringify({bundle:{createUpdaterArtifacts:true}})],{env:{...process.env,TAURI_SIGNING_PRIVATE_KEY:key,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD??''}});
const bundle=join(root,'src-tauri/target/release/bundle/macos');
const app=join(bundle,'Branchline.app');
run('/usr/bin/codesign',['--verify','--deep','--strict',app]);
const appVersion=run('/usr/libexec/PlistBuddy',['-c','Print :CFBundleShortVersionString',join(app,'Contents/Info.plist')],{stdio:['ignore','pipe','inherit']});
if(appVersion!==version)throw new Error('The built app version does not match the release.');
const identifier=run('/usr/libexec/PlistBuddy',['-c','Print :CFBundleIdentifier',join(app,'Contents/Info.plist')],{stdio:['ignore','pipe','inherit']});
if(identifier!==config.identifier)throw new Error('The built app identity does not match the release.');
const architecture=run('/usr/bin/lipo',['-archs',join(app,'Contents/MacOS/branchline')],{stdio:['ignore','pipe','inherit']});
if(architecture!=='arm64')throw new Error('The updater archive requires an Apple Silicon build.');
const signature=readFileSync(join(bundle,'Branchline.app.tar.gz.sig'),'utf8');
const manifest=releaseManifest({version,notes,signature});
mkdirSync(destination,{recursive:true});
for(const file of ['Branchline.app.tar.gz','Branchline.app.tar.gz.sig'])copyFileSync(join(bundle,file),join(destination,file));
writeFileSync(join(destination,'latest.json'),JSON.stringify(manifest,null,2)+'\n');
const zip=join(destination,'Branchline-macOS-arm64.zip');
run('/usr/bin/ditto',['-c','-k','--sequesterRsrc','--keepParent',app,zip]);
const checksum=createHash('sha256').update(readFileSync(zip)).digest('hex');
writeFileSync(`${zip}.sha256`,`${checksum}  Branchline-macOS-arm64.zip\n`);
console.log(`Release files prepared in ${destination}. Publish all five files with the matching v${version} GitHub release. Nothing has been uploaded.`);

try {
 const cleanup=cleanupReleaseCaches({root,version,releaseDirectory:destination});
 console.log(`Removed ${cleanup.removed.length} generated build cache directories; app and release assets retained.`);
}catch(error){
 console.warn(`Release assets are ready, but cache cleanup was skipped: ${error.message}`);
}
