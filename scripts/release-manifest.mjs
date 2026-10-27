export function releaseManifest({version,notes,signature}) {
  if(!/^\d+\.\d+\.\d+$/.test(version))throw new Error('A stable release version is required.');
  if(typeof notes!=='string'||!notes.trim())throw new Error('Release notes are required.');
  if(typeof signature!=='string'||!signature.trim())throw new Error('A signed updater archive is required.');
  const trusted=Buffer.from(signature.trim(),'base64').toString('utf8').split('\n').find(line=>line.startsWith('trusted comment:'));
  if(!trusted?.split('\t').includes(`version:${version}`))throw new Error('The update signature must bind the announced version. Use a current Tauri CLI.');
  return {
    version,
    notes,
    platforms:{'darwin-aarch64':{
      signature:signature.trim(),
      url:`https://github.com/ShindouMihou/branchline/releases/download/v${version}/Branchline.app.tar.gz`,
    }},
  };
}
