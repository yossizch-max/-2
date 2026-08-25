import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),"..");
function walk(dir){
  if(!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir,{withFileTypes:true}).flatMap(e=>{
    const p=path.join(dir,e.name);
    return e.isDirectory()?walk(p):[p];
  });
}
const ts=walk(path.join(root,"src")).filter(x=>/\.(ts|tsx)$/.test(x)).map(x=>fs.readFileSync(x,"utf8")).join("\n");
const rust=walk(path.join(root,"src-tauri","src")).filter(x=>x.endsWith(".rs")).map(x=>fs.readFileSync(x,"utf8")).join("\n");

const invokes=new Set([...ts.matchAll(/call(?:<[^>]+>)?\(\s*"([^"]+)"/g)].map(m=>m[1]));
const commands=new Set([...rust.matchAll(/#\[tauri::command\]\s*pub fn\s+([A-Za-z0-9_]+)/g)].map(m=>m[1]));
const missing=[...invokes].filter(x=>!commands.has(x)).sort();
const extra=[...commands].filter(x=>!invokes.has(x)).sort();

const result={
  frontendInvokeCount:invokes.size,
  rustCommandCount:commands.size,
  missing,
  extra
};
console.log(JSON.stringify(result,null,2));
// missing/extra empty already implies invokes and commands are the same set (and
// therefore the same size) - no need to also hardcode an expected count here, which
// would just have to be bumped by hand every time a command is added or removed.
if(missing.length || extra.length) process.exit(1);
