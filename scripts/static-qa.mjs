import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),"..");
const read=p=>fs.readFileSync(path.join(root,p),"utf8");

const palette=read("src/components/CommandPalette.tsx");
const shell=read("src/components/AppShell.tsx");
const tokens=read("src/styles/tokens.css");
const sql=read("src-tauri/migrations/001_schema_v12.sql");
const snapshot=read("src-tauri/src/source_snapshot.rs");
const extraction=read("src-tauri/src/extraction.rs");
const ai=read("src-tauri/src/ai.rs");

const checks={
  commandPaletteDialog:palette.includes('role="dialog"')&&palette.includes('aria-modal="true"'),
  commandPaletteFocusTrap:palette.includes('e.key!=="Tab"')&&palette.includes("openerRef"),
  ariaCurrent:shell.includes("aria-current"),
  focusVisible:tokens.includes(":focus-visible"),
  thirtyOneTables:(sql.match(/CREATE TABLE /g)||[]).length===31,
  ocrSourceMismatch:snapshot.includes("SourceShaMismatch"),
  ocrReverify:extraction.includes("snapshot.verify_unchanged()"),
  docxDocumentAnchoring:extraction.includes('anchor_kind: "document"')&&extraction.includes("page_number: None"),
  aiFixedOpenAiEndpoint:ai.includes("https://api.openai.com/v1/responses"),
  aiLoopbackGate:ai.includes("local provider must use loopback"),
  aiNoRawProviderPersistence:!ai.includes("response.text()"),
  aiStoreFalse:ai.includes('"store":false'),
  aiBackgroundFalse:ai.includes('"background":false')
};

console.log(JSON.stringify(checks,null,2));
if(Object.values(checks).some(x=>!x)) process.exit(1);
