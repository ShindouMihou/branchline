import {existsSync,lstatSync,readFileSync,realpathSync,rmSync} from 'node:fs';
import {basename,dirname,isAbsolute,join,relative,resolve,sep} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';

function hasActiveBuilds(){
 const result=spawnSync('ps',['-Ao','comm='],{encoding:'utf8'});
 if(result.error||result.status!==0)throw new Error('Cannot check for active Rust builds; caches were kept.');
 return result.stdout.split('\n').some(command=>['cargo','rustc','rustdoc','rust-lld'].includes(basename(command.trim())));
}
function inside(path,directory){const part=relative(directory,path);return part===''||part!=='..'&&!part.startsWith(`..${sep}`)&&!isAbsolute(part)}
function metadata(path){
 try{return lstatSync(path)}catch(error){if(error.code==='ENOENT')return null;throw error;}
}
function noLinkedParents(root,path){
 let current=root;
 for(const part of relative(root,path).split(sep)){
  current=join(current,part);
  if(metadata(current)?.isSymbolicLink())throw new Error(`Cache cleanup refuses symbolic links: ${current}`);
 }
}
function validateAssets(root,directory,version){
 if(!/^\d+\.\d+\.\d+$/.test(version))throw new Error('Invalid release version; caches were kept.');
 const config=JSON.parse(readFileSync(join(root,'src-tauri/tauri.conf.json'),'utf8'));
 const pkg=JSON.parse(readFileSync(join(root,'package.json'),'utf8'));
 if(pkg.name!=='branchline'||pkg.version!==version||config.version!==version)throw new Error('Release versions do not match; caches were kept.');
 const bundle=join(root,'src-tauri/target/release/bundle/macos/Branchline.app');
 const binary=join(bundle,'Contents/MacOS/branchline');
 if(!lstatSync(binary).isFile()||!lstatSync(binary).size)throw new Error('The retained app is missing; caches were kept.');
 const manifest=JSON.parse(readFileSync(join(directory,'latest.json'),'utf8'));
 const platform=manifest.platforms?.['darwin-aarch64'];
 const signature=readFileSync(join(directory,'Branchline.app.tar.gz.sig'),'utf8').trim();
 if(manifest.version!==version||!signature||platform?.signature?.trim()!==signature||platform.url!==`https://github.com/ShindouMihou/branchline/releases/download/v${version}/Branchline.app.tar.gz`)throw new Error('Release updater assets do not match; caches were kept.');
 for(const name of ['Branchline.app.tar.gz','Branchline-macOS-arm64.zip']){
  const file=join(directory,name);if(!lstatSync(file).isFile()||!lstatSync(file).size)throw new Error(`Missing release asset: ${name}`);
 }
 const checksum=readFileSync(join(directory,'Branchline-macOS-arm64.zip.sha256'),'utf8').trim().split(/\s+/);
 const actual=createHash('sha256').update(readFileSync(join(directory,'Branchline-macOS-arm64.zip'))).digest('hex');
 if(checksum[0]!==actual||checksum[1]!=='Branchline-macOS-arm64.zip')throw new Error('Release ZIP checksum does not match; caches were kept.');
 return [realpathSync(bundle),directory];
}

export function cleanupReleaseCaches({root,releaseDirectory,version,buildsRunning=hasActiveBuilds}){
 root=realpathSync(root);releaseDirectory=realpathSync(releaseDirectory);
 const retained=validateAssets(root,releaseDirectory,version);
 const caches=['src-tauri/target/debug','src-tauri/target/release/build','src-tauri/target/release/deps','src-tauri/target/release/.fingerprint','src-tauri/target/release/incremental'].map(path=>join(root,path));
 for(const path of caches){
  noLinkedParents(root,path);
  if(retained.some(keep=>inside(keep,path)||inside(path,keep)))throw new Error('A retained app or release directory overlaps a build cache; cleanup stopped.');
  if(existsSync(path)&&!lstatSync(path).isDirectory())throw new Error(`Expected a cache directory: ${path}`);
 }
 if(buildsRunning())throw new Error('A Rust build is active; caches were kept. Retry npm run release:cleanup after it finishes.');
 const removed=[];
 for(const path of caches){
  if(!existsSync(path))continue;
  noLinkedParents(root,path);rmSync(path,{recursive:true});removed.push(relative(root,path));
 }
 return {removed,retained};
}

const entry=process.argv[1]&&resolve(process.argv[1]);
if(entry===fileURLToPath(import.meta.url)){
 try{
  const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
  const {version}=JSON.parse(readFileSync(join(root,'package.json'),'utf8'));
  const args=process.argv.slice(2);
  if(args.length&&!(args.length===2&&args[0]==='--output'))throw new Error('Usage: npm run release:cleanup -- [--output release-directory]');
  const releaseDirectory=args.length?resolve(root,args[1]):join(root,'.local/releases',`v${version}`);
  const result=cleanupReleaseCaches({root,version,releaseDirectory});
  console.log(`Removed ${result.removed.length} generated cache directories. The app, release assets, toolchain, and source are retained.`);
 }catch(error){console.error(error.message);process.exitCode=1;}
}
