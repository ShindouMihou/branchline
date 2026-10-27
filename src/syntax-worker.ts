import {highlightSource} from './syntax-engine';
let queue=Promise.resolve(),latest=0;
self.onmessage=event=>{const {id,language,before,after}=event.data;latest=id;queue=queue.then(async()=>{
  if(id!==latest){self.postMessage({id,before:[],after:[]});return;}
  try {const oldLines=await highlightSource(before??'',language);if(id!==latest){self.postMessage({id,before:[],after:[]});return;}const newLines=await highlightSource(after??'',language);self.postMessage({id,before:oldLines,after:newLines});}
  catch {self.postMessage({id,error:true,before:[],after:[]});}
});};
