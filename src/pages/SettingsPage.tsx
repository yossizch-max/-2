import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { LegalRulesPage } from "./LegalRulesPage";
import type { LegalRuleset } from "../types";

type Suggestion={id:string;pathDisplay:string;suggestedTitle:string;fileCount:number;createdAt:string};
type ScanRun={id:string;rootPath:string;status:string;startedAt:string;finishedAt?:string|null;discoveredCount:number;hashedCount:number;errorCount:number};

export function SettingsPage({onNavigate}:{onNavigate?:(target:string)=>void}) {
  const [showLegalRules,setShowLegalRules]=useState(false);
  const {data:rulesets}=useCommand(
    ()=>commands.list_legal_rulesets({}) as Promise<LegalRuleset[]>, []
  );
  const {data:officeRoot,reload:reloadRoot}=useCommand(
    ()=>commands.get_office_root() as Promise<{path:string|null}>, []
  );
  const {data:suggestions,reload:reloadSuggestions}=useCommand(
    ()=>commands.list_matter_suggestions() as Promise<Suggestion[]>, []
  );
  const {data:scanRuns,reload:reloadScanRuns}=useCommand(
    ()=>commands.list_scan_runs() as Promise<ScanRun[]>, []
  );
  const [busy,setBusy]=useState<string|null>(null);
  const [status,setStatus]=useState<string|null>(null);

  const chooseAndSetRoot=async()=>{
    setBusy("choose");setStatus(null);
    try{
      const picked=await commands.choose_folder() as {path:string|null};
      if(picked.path){
        await commands.set_office_root({path:picked.path});
        reloadRoot();
      }
    }catch(e){setStatus(String(e));}
    finally{setBusy(null);}
  };
  const scan=async()=>{
    setBusy("scan");setStatus(null);
    try{
      await commands.scan_office_root();
      reloadSuggestions();
      reloadScanRuns();
      setStatus("הסריקה הושלמה.");
    }catch(e){setStatus(String(e));}
    finally{setBusy(null);}
  };
  const createFromSuggestion=async(s:Suggestion)=>{
    setBusy(s.id);setStatus(null);
    try{
      const matter=await commands.create_matter({title:s.suggestedTitle}) as {id:string};
      await commands.bind_existing_matter({suggestionId:s.id,matterId:matter.id});
      reloadSuggestions();
    }catch(e){setStatus(String(e));}
    finally{setBusy(null);}
  };
  const reject=async(s:Suggestion)=>{
    setBusy(s.id);
    try{ await commands.reject_matter_suggestion({suggestionId:s.id}); reloadSuggestions(); }
    finally{ setBusy(null); }
  };

  const rows=[["מסד נתונים","SQLCipher · תקין"],["מפתח שחזור","Restore drill חובה לפני שימוש אמיתי"],["סריקה","Stage A / Stage B · Local-first"],["OCR","Tesseract + Poppler + heb/ara/eng"],["PDF Export","Word/LibreOffice נדרש"],["Code signing","חובה לפני הפצה"]];

  if(showLegalRules){
    return <div className="page"><LegalRulesPage onBack={()=>setShowLegalRules(false)}/></div>;
  }

  const approvedRulesetCount=rulesets?.filter(r=>r.status==="approved").length??0;

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">SYSTEM</span><h1>הגדרות ובריאות</h1><p>הצפנה, שחזור, סריקה, OCR, Rulesets ו־AI.</p></div></div>

    {onNavigate && <section className="workspace-card">
      <h2>כלים נוספים</h2>
      <p className="quiet">AI ותבניות הן יכולות שנפתחות מכאן או מ־Ctrl+K, לא יעדי ניווט קבועים.</p>
      <div className="header-actions">
        <button className="btn secondary" onClick={()=>onNavigate("ai")}>הגדרות AI</button>
        <button className="btn secondary" onClick={()=>onNavigate("templates")}>תבניות</button>
        <button className="btn secondary" onClick={()=>onNavigate("actions")}>מרכז פעולה</button>
      </div>
    </section>}

    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">GOVERNANCE</span><h2>כללים משפטיים</h2></div>
        <button className="btn secondary" onClick={()=>setShowLegalRules(true)}>פתח</button></div>
      <p className="quiet">{rulesets?.length??0} Rulesets · {approvedRulesetCount} מאושרים. כלל דטרמיניסטי לחישוב מועד או פיצוי נכנס לשימוש רק דרך Ruleset מאושר, עם מקור מאומת ובדיקות עוברות.</p>
    </section>

    <section className="workspace-card">
      <h2>תיקיית משרד (Office Root)</h2>
      <p className="quiet">{officeRoot?.path ?? "לא נבחרה עדיין תיקיית משרד."}</p>
      {status && <p className="quiet">{status}</p>}
      <div className="header-actions">
        <button className="btn secondary" onClick={chooseAndSetRoot} disabled={busy==="choose"}>{busy==="choose"?"בוחר...":"בחר תיקייה"}</button>
        <button className="btn primary" onClick={scan} disabled={!officeRoot?.path||busy==="scan"}>{busy==="scan"?"סורק...":"סרוק עכשיו"}</button>
      </div>
    </section>

    {suggestions && suggestions.length>0 && <section className="workspace-card">
      <h2>תיקיות שהתגלו וטרם שויכו</h2>
      <p className="quiet">תיקיות אלה נמצאו בסריקה אך אינן משויכות לתיק קיים.</p>
      {suggestions.map(s=><div className="tr" key={s.id}>
        <span><b>{s.suggestedTitle}</b><small>{s.pathDisplay}</small></span>
        <span>{s.fileCount} קבצים</span>
        <span>
          <button className="btn secondary" onClick={()=>reject(s)} disabled={busy===s.id}>התעלם</button>
          <button className="btn primary" onClick={()=>createFromSuggestion(s)} disabled={busy===s.id}>{busy===s.id?"יוצר...":"צור תיק"}</button>
        </span>
      </div>)}
    </section>}

    {scanRuns && scanRuns.length>0 && <section className="workspace-card">
      <h2>היסטוריית סריקות</h2>
      {scanRuns.map(r=><div className="tr" key={r.id}>
        <span>{r.startedAt}</span>
        <span>{r.status}</span>
        <span>{r.discoveredCount} קבצים · {r.hashedCount} עובדו{r.errorCount>0?` · ${r.errorCount} שגיאות`:""}</span>
      </div>)}
    </section>}

    <div className="settings-list">{rows.map(([a,b])=><div className="setting-row" key={a}><strong>{a}</strong><span>{b}</span></div>)}</div>
  </div>;
}
