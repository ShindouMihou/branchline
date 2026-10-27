import {test} from 'node:test';
import assert from 'node:assert/strict';
import {releaseManifest} from '../scripts/release-manifest.mjs';

test('the updater manifest binds the release version to its signed Apple Silicon asset',()=>{
 const manifest=releaseManifest({version:'0.5.0',notes:'### Changes\n\n- Update',signature:Buffer.from('trusted comment: timestamp:1\tversion:0.5.0').toString('base64')});
 assert.equal(manifest.version,'0.5.0');
 assert.equal(typeof manifest.platforms['darwin-aarch64'].signature,'string');
 assert.equal(manifest.platforms['darwin-aarch64'].url,'https://github.com/ShindouMihou/branchline/releases/download/v0.5.0/Branchline.app.tar.gz');
 assert.equal(Object.keys(manifest.platforms).length,1);
});
test('incomplete or unsafe release metadata cannot become an update manifest',()=>{
 for(const version of ['v0.5.0','../main','0.5.0-beta.1',''])assert.throws(()=>releaseManifest({version,notes:'Notes',signature:'sig'}));
 assert.throws(()=>releaseManifest({version:'0.5.0',notes:'',signature:'sig'}));
 assert.throws(()=>releaseManifest({version:'0.5.0',notes:'Notes',signature:''}));
 assert.throws(()=>releaseManifest({version:'0.5.0',notes:'Notes',signature:Buffer.from('trusted comment: timestamp:1\tversion:0.4.0').toString('base64')}));
});
