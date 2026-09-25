// Run against the shipped visualizer artifacts with fixture navigation; no MAP backend is required.
// MAP_PLAYWRIGHT_MODULE may point to an installed Playwright module.
// MAP_BROWSER_EXECUTABLE selects a Chromium-family browser. Prefer installed Brave on macOS.
// Run native browser checks outside restricted execution sandboxes on macOS.
// MAP_COMPRESSION_SCREENSHOT optionally saves the final narrow-viewport fixture.
const { chromium } = await import(process.env.MAP_PLAYWRIGHT_MODULE ?? 'playwright');
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
const root = fileURLToPath(new URL('..', import.meta.url));
const theme = await readFile(`${root}/host/ui/src/launcher-theme.generated.css`, 'utf8');
const sources = {};
for (const name of ['path-inspector', 'holon-inspector']) sources[name] = 'data:text/javascript;base64,' + Buffer.from(await readFile(`${root}/host/conductora/resources/dahn-visualizers/${name}.js`)).toString('base64');
const braveExecutable = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const executablePath = process.env.MAP_BROWSER_EXECUTABLE
  ?? (process.platform === 'darwin' && existsSync(braveExecutable) ? braveExecutable : undefined);
const browser = await chromium.launch({ executablePath, headless: true });
try {
const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
const pageErrors = [];
page.on('pageerror', error => pageErrors.push(error.message));
await page.setContent(`<style>${theme} *{box-sizing:border-box} body{margin:0;background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color);font-family:system-ui} #host{height:720px;display:flex;padding:20px;overflow:hidden}</style><main id="host"></main>`);
await page.evaluate(async sources => {
 for (const [name, url] of Object.entries(sources)) customElements.define(`check-${name}`, (await import(url)).default);
 const make = (id, rowId, column, parent, kind='singular-relationship') => {
   const element = document.createElement('check-holon-inspector');
   const properties = document.createElement('section'); properties.innerHTML = '<label>Retained input <input value="original"></label><p>Properties remain mounted.</p>';
   const collection = document.createElement('section'); collection.innerHTML = '<table><tr><td>Selected collection member</td><td>Another column</td></tr></table>'; collection.dataset.selectedRow='one';
   const affordance = {kind:'relationship',label:'Author'};
   element.setContext({title:`Theme: ${id}.BootstrapTheme`,holonKey:`${id}.BootstrapTheme`,activateRelationship:()=>{},nodeAffordances:{singularRelationships:[affordance],collections:[]},childVisualizers:new Map([['properties',properties],['collections',collection]])});
   element.setSingularNavigationState({state:'loaded',active:affordance});
   return {id,rowId,column,element,provenance:parent?{parentOccurrenceId:parent,kind}:undefined};
 };
 const a=make('A','r0',1), b=make('B','r0',2,'A'), c=make('C','r0',3,'B'), down=make('D','r1',3,'C','collection-member');
 window.items=[a,b,c,down];
 const path=document.createElement('check-path-inspector');window.path=path;
 window.focus={occurrenceId:'C',mode:'traverse'};
 path.setContext({navigation:{subscribe(render){window.publish=render;render(window.items,window.focus);return()=>{}},restore(id){window.focus={occurrenceId:id,mode:'restore'};window.publish(window.items,window.focus)},dispose(){}}});
 document.querySelector('#host').append(path);
 window.restore=id=>path.navigation.restore(id);
}, sources);
await page.waitForTimeout(200);
const report = await page.evaluate(async () => {
 const assert=(value,message)=>{if(!value)throw Error(message)};
 const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
 const result=[];
 const containment=()=>{
  for(const item of items){const region=item.element.parentElement.getBoundingClientRect();const node=item.element.getBoundingClientRect();assert(node.width<=region.width+1,`${item.id} exceeds column`);assert(node.height<=region.height+1,`${item.id} exceeds row`);
   const subordinate=item.element.collectionRegion.getBoundingClientRect();assert(subordinate.width<=node.width+1,`${item.id} collection exceeds Node`);}
 };
 containment();
 const initialNodes = items.map(item => item.element);
 const aInitial = items[0].element;
 assert(aInitial.titleControl.textContent === 'A.BootstrapTheme', 'Compressed title must show only the key');
 assert(aInitial.titleControl.style.writingMode === 'vertical-rl', 'X-only compact identity must use a vertical presentation');
 assert(!items[1].element.body.inert && items[1].element.propertyViewer.inert, 'Partial X must retain the rail while properties yield width');
 assert(items[3].element.body.inert, 'Y-only occurrence must yield body height');
 result.push('Initial mixed-axis column/row containment');
 assert(path.columnWidths[0]<path.columnWidths[1]&&path.columnWidths[1]<path.columnWidths[2],'Expected compact/partial/full column widths');
 const a=items[0].element;
 restore('D');await settle();containment();
 assert(items.every((item, index) => item.element === initialNodes[index]), 'Compression remounted a Node');
 assert(a.titleControl.textContent === 'ABT', 'XY title must show key initials');
 assert(a.titleControl.scrollWidth <= a.titleControl.clientWidth, 'Short initials must fit the compact title');
 assert(a.body.inert&&a.collectionRegion.inert,'XY content must be inert');
 assert(a.titleControl.getBoundingClientRect().height>=40,'XY restore target too short');
 a.titleControl.focus();

 result.push('XY presentation hides subordinate controls while preserving restore target');
 return result;
});
await page.keyboard.press('Enter'); await page.waitForTimeout(100);
await page.evaluate(()=>{if(path.focus.occurrenceId!=='A'||items[0].element.body.inert)throw Error('Keyboard restore failed');});
report.push('Keyboard restoration expands both axes');
await page.evaluate(()=>{const a=items[0].element;const input=a.querySelector('input');input.value='preserved';input.focus();restore('D');if(document.activeElement!==a.titleControl)throw Error('Focus trapped in hidden content');restore('A');if(input.value!=='preserved')throw Error('Local state lost');});
report.push('Focus relocation and local-state restoration');
await page.setViewportSize({width:560,height:600});await page.waitForTimeout(100);
await page.evaluate(()=>{for(const item of items){const node=item.element.getBoundingClientRect(), region=item.element.parentElement.getBoundingClientRect();if(node.width>region.width+1)throw Error('Resize escaped column')}if(path.querySelectorAll('[data-lineage-child]').length!==3)throw Error('Lineage lost');restore('D');});
await page.waitForTimeout(100);
if (process.env.MAP_COMPRESSION_SCREENSHOT) await page.screenshot({path:process.env.MAP_COMPRESSION_SCREENSHOT,fullPage:true});
report.push('Narrow viewport retains lineage and constrained presentation');
if (pageErrors.length) throw new Error(pageErrors.join('\n'));
console.log(JSON.stringify(report,null,2));
} finally { await browser.close(); }
