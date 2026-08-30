import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { commands } from "../lib/ipc";
import type { DocumentIntakeSummary } from "../types";

// UX Milestone 1: direct document intake. A file dropped or picked here never stays
// dependent on where it came from - the backend copies it into a TAHRIR-managed
// location, verifies the copy by hash, then runs the same local extraction/OCR/
// classification pipeline the Office Root scanner already uses. No cloud/AI call is
// made anywhere in this flow. This is the single canonical intake path - both
// Matter Home and the Documents screen render this same component, never a
// second copy of the import logic.
export function DirectIntakeZone({matterId,onImported}:{matterId:string;onImported:()=>void}) {
  const [dragOver,setDragOver]=useState(false);
  const [busy,setBusy]=useState(false);
  const [summary,setSummary]=useState<DocumentIntakeSummary&{imported?:number;importErrors?:{fileName:string;errorMessage:string}[]}|null>(null);
  const [error,setError]=useState<string|null>(null);
  const dragCounter=useRef(0);

  const runImport=async(paths:string[])=>{
    try{
      if(paths.length===0)return;
      setBusy(true);setError(null);setSummary(null);
      const result=await commands.import_document_files({matterId,paths}) as DocumentIntakeSummary;
      setSummary(result);
      onImported();
    }catch(e){ setError(String(e)); }
    finally{ setBusy(false); setDragOver(false); dragCounter.current=0; }
  };

  const chooseFiles=async()=>{
    setError(null);
    try{
      const picked=await commands.choose_document_files() as {paths:string[]};
      if(picked.paths?.length) await runImport(picked.paths);
    }catch(e){ setError(String(e)); }
  };

  // A dragged file must never be able to take the whole app down with it: this
  // effect never lets an exception inside the native drag-drop callback escape
  // uncaught, and it never leaks a stale listener registration. `getCurrentWebview
  // ().onDragDropEvent` resolves asynchronously (it subscribes to four separate
  // Tauri events internally) - if this component unmounts (matter/tab switch)
  // before that promise settles, the old code left `unlisten` as `undefined`
  // forever, so the listener it eventually registered was never released. Opening
  // several matters in a row could then leave multiple stale drag-drop listeners
  // all still firing on the current webview at once. `cancelled` closes that race:
  // if we've already unmounted by the time registration resolves, unlisten
  // immediately instead of leaving it dangling.
  useEffect(()=>{
    let cancelled=false;
    let unlisten:(()=>void)|undefined;
    getCurrentWebview().onDragDropEvent((event)=>{
      try{
        const payload=event.payload as {type:string;paths?:string[]};
        if(payload.type==="enter"||payload.type==="over"){ dragCounter.current+=1; setDragOver(true); }
        else if(payload.type==="leave"){ dragCounter.current=0; setDragOver(false); }
        else if(payload.type==="drop"){ dragCounter.current=0; setDragOver(false); void runImport(payload.paths??[]); }
      }catch(e){ setError(String(e)); setDragOver(false); dragCounter.current=0; }
    }).then(fn=>{
      if(cancelled){ fn(); return; }
      unlisten=fn;
    }).catch(e=>{ setError(String(e)); });
    return ()=>{ cancelled=true; unlisten?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  },[matterId]);

  return <section className={`workspace-card intake-zone${dragOver?" drag-over":""}`}>
    <div className="header-actions" style={{justifyContent:"space-between"}}>
      <div><span className="eyebrow">קליטת מסמכים</span><h2>גרור מסמכים לכאן</h2></div>
      <button className="btn secondary" onClick={chooseFiles} disabled={busy}>בחר קבצים</button>
    </div>
    <p className="quiet">
      גרירה או בחירה של קובץ יוצרת עותק מנוהל של התיק, ואז קוראת אותו אוטומטית ברקע (כולל OCR וסיווג) —
      אין תלות בתיקיית Downloads/Desktop המקורית, ואין צורך בסריקת תיקיית משרד.
    </p>
    {busy && <p className="quiet">קורא מסמכים...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {summary && <div className="workspace-card" style={{marginTop:8}}>
      <strong>
        נקלטו {summary.imported ?? 0} מסמכים
        {summary.extracted?` · ${summary.extracted} נקראו`:""}
        {summary.ocred?` · ${summary.ocred} עם OCR`:""}
        {summary.classified?` · ${summary.classified} סווגו`:""}
      </strong>
      {(summary.importErrors?.length??0)>0 && <div style={{marginTop:6}}>
        <p className="quiet">{summary.importErrors!.length} קבצים לא נקלטו:</p>
        {summary.importErrors!.map((e,i)=><p className="quiet" key={i} style={{margin:"2px 0"}}>
          <strong>{e.fileName}</strong> — {e.errorMessage}
        </p>)}
      </div>}
      {(summary.failed??0)>0 && <p className="quiet" style={{marginTop:6}}>{summary.failed} מסמכים דורשים טיפול — ראו רשימת המסמכים למטה.</p>}
    </div>}
  </section>;
}
