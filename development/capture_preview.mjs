import { createRequire } from 'node:module';
import fs from 'node:fs';
const require=createRequire(import.meta.url);
const options=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const {chromium}=require(options.playwright_module);
const admitted=new URL(options.url);
let browser;
let context;
const network_blocks={external_origin:0,write_method:0,websocket:0};
const deadline=setTimeout(()=>{ if(context)context.close().finally(()=>process.exit(2));else process.exit(2); },25000);
try {
 context=await chromium.launchPersistentContext(options.profile,{headless:true,executablePath:options.browser_executable,timeout:10000,args:['--no-first-run','--disable-background-networking'],viewport:{width:1280,height:800},deviceScaleFactor:1,serviceWorkers:'block'});
 browser=context.browser();
 await context.route('**/*',async route=>{
  const request=new URL(route.request().url());
  if(['GET','HEAD'].includes(route.request().method()) && request.origin===admitted.origin && !request.username && !request.password)await route.continue();
  else {network_blocks[request.origin!==admitted.origin?'external_origin':'write_method']++;await route.abort('blockedbyclient');}
 });
 await context.routeWebSocket('**/*',ws=>{network_blocks.websocket++;ws.close();});
 const page=await context.newPage();
 const errors=[];
 page.on('pageerror',e=>errors.push(e.name));
 const response=await page.goto(options.url,{waitUntil:'load',timeout:15000});
 if(!response||!response.ok())throw new Error('preview_response_failed');
 if(new URL(page.url()).origin!==admitted.origin)throw new Error('preview_redirect_denied');
 await page.evaluate(async()=>{
  await document.fonts.ready;
  await Promise.all([...document.images].filter(image=>image.complete).map(image=>image.decode().catch(()=>{})));
 });
 await page.screenshot({path:options.output,type:'png',fullPage:false,timeout:10000});
 console.log(JSON.stringify({browser_version:browser.version(),url:page.url(),viewport:{width:1280,height:800},status:response.status(),page_error_names:errors,network_blocks,screenshot:'actual_browser_viewport'}));
} finally {clearTimeout(deadline);if(context)await context.close();}
