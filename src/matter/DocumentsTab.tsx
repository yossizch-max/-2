import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { DocumentRow } from "../types";

const CATEGORIES = ["general","medical","court","wage","correspondence","expert_opinion"];

export function DocumentsTab({matterId}:{matterId:string}) {
  const {data:documents,loading,error,reload}=useCommand(
    ()=>commands.list_documents({matterId}) as Promise<DocumentRow[]>, [matterId]
  );
  const [busy,setBusy]=useState<string|null>(null);

  const scan=async()=>{
    setBusy("scan");
    try{ await commands.hash_pending_files({matterId}); reload(); }
    finally{ setBusy(null); }
  };
  const extract=async(documentId:string)=>{
    setBusy(documentId);
    try{ await commands.extract_document_text({documentId}); reload(); }
    finally{ setBusy(null); }
  };
  const openFile=async(occurrenceId:string)=>{
    setBusy(occurrenceId);
    try{ await commands.open_occurrence({occurrenceId}); }
    finally{ setBusy(null); }
  };
  const revealFile=async(occurrenceId:string)=>{
    setBusy(occurrenceId);
    try{ await commands.reveal_occurrence({occurrenceId}); }
    finally{ setBusy(null); }
  };
  const changeCategory=async(documentId:string,category:string)=>{
    setBusy(documentId);
    try{ await commands.classify_document_manual({matterId,documentId,category}); reload(); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">SOURCE GRAPH</span><h2>מסמכים</h2></div><button className="btn primary" onClick={scan} disabled={busy==="scan"}>{busy==="scan"?"סורק...":"סרוק ועדכן"}</button></div>
    {loading && <p className="quiet">טוען מסמכים...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && documents?.length===0 && <p className="quiet">אין עדיין מסמכים בתיק זה. יש לוודא שהתיקייה משויכת ולסרוק.</p>}
    <div className="table">
      <div className="tr th"><span>קובץ</span><span>קטגוריה</span><span>מצב מקור</span><span>טקסט</span><span>פעולות</span></div>
      {documents?.map(d=><div className="tr" key={d.id}>
        <span><b>{d.fileName}</b><small>{d.modifiedAt}</small></span>
        <span>
          <select value={d.category} disabled={busy===d.id} onChange={e=>changeCategory(d.id,e.target.value)}>
            {CATEGORIES.map(c=><option key={c} value={c}>{c}</option>)}
          </select>
        </span>
        <span><StatusBadge tone={d.sourceState==="local"?"ok":"warn"}>{d.sourceState}</StatusBadge></span>
        <span>{d.extractionState!=="complete"
          ? <button className="btn secondary" onClick={()=>extract(d.id)} disabled={busy===d.id}>{busy===d.id?"מחלץ...":`חלץ (${d.extractionState})`}</button>
          : d.extractionState}
        </span>
        <span>
          {d.occurrenceId && <>
            <button className="btn secondary" onClick={()=>openFile(d.occurrenceId!)} disabled={busy===d.occurrenceId}>פתח</button>
            <button className="btn secondary" onClick={()=>revealFile(d.occurrenceId!)} disabled={busy===d.occurrenceId}>הצג בתיקייה</button>
          </>}
        </span>
      </div>)}
    </div>
  </section>;
}
