import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { DocumentRow, DocumentIntakeSummary, ExtractionRun, DocumentPage } from "../types";

const CATEGORIES = ["general","medical","court","wage","correspondence","expert_opinion"];
const CATEGORY_LABELS: Record<string,string> = {
  general: "כללי", medical: "רפואי", court: "בית משפט", wage: "שכר",
  correspondence: "התכתבות", expert_opinion: "חוות דעת",
};
const ERROR_LABELS: Record<string,string> = {
  runtime_missing: "רכיב OCR חסר בהתקנה", unsupported_format: "סוג קובץ לא נתמך לחילוץ טקסט",
  pdftotext_failed: "חילוץ טקסט מ-PDF נכשל", rasterization_failed: "המרת PDF לתמונה נכשלה",
  ocr_failed: "זיהוי טקסט (OCR) נכשל", source_changed: "הקובץ השתנה במהלך העיבוד - יש לסרוק שוב",
  persistence_failed: "שמירה במסד הנתונים נכשלה",
};
function errorLabel(code?:string|null){ return code ? (ERROR_LABELS[code]??code) : "שגיאה לא ידועה"; }

function DocumentDetail({documentVersionId}:{documentVersionId:string}) {
  const {data:pages,loading:pagesLoading}=useCommand(
    ()=>commands.get_document_pages({documentVersionId}) as Promise<DocumentPage[]>, [documentVersionId]
  );
  const {data:runs,loading:runsLoading}=useCommand(
    ()=>commands.list_extraction_runs({documentVersionId}) as Promise<ExtractionRun[]>, [documentVersionId]
  );
  const [expandedPage,setExpandedPage]=useState<string|null>(null);
  return <div className="document-detail">
    <div className="grid-2">
      <section>
        <h4>עמודים/בלוקים שחולצו</h4>
        {pagesLoading && <p className="quiet">טוען...</p>}
        {!pagesLoading && pages?.length===0 && <p className="quiet">טרם חולץ טקסט עבור גרסה זו.</p>}
        {pages?.map(p=><div key={p.id} className="mini-row" style={{cursor:"pointer"}} onClick={()=>setExpandedPage(expandedPage===p.id?null:p.id)}>
          <span>{p.anchorKind==="page"?`עמוד ${p.pageNumber}`:"מסמך שלם"} · {p.method}</span>
          <small>{p.text.length.toLocaleString()} תווים · {p.textSha256.slice(0,8)}</small>
          {expandedPage===p.id && <p className="quiet" style={{whiteSpace:"pre-wrap",maxHeight:220,overflowY:"auto"}}>{p.text}</p>}
        </div>)}
      </section>
      <section>
        <h4>יומן ניסיונות חילוץ</h4>
        {runsLoading && <p className="quiet">טוען...</p>}
        {runs?.map(r=><div key={r.id} className="mini-row">
          <span><StatusBadge tone={r.status==="completed"?"ok":r.status==="failed"?"risk":"warn"}>{r.status}</StatusBadge> {r.startedAt}</span>
          <small>{r.errorCode?errorLabel(r.errorCode):r.finishedAt?"הושלם":"בתהליך"}</small>
        </div>)}
        {runs?.length===0 && <p className="quiet">אין עדיין ניסיונות חילוץ.</p>}
      </section>
    </div>
  </div>;
}

export function DocumentsTab({matterId}:{matterId:string}) {
  const {data:documents,loading,error,reload}=useCommand(
    ()=>commands.list_documents({matterId}) as Promise<DocumentRow[]>, [matterId]
  );
  const [busy,setBusy]=useState<string|null>(null);
  const [summary,setSummary]=useState<DocumentIntakeSummary|null>(null);
  const [processError,setProcessError]=useState<string|null>(null);
  const [expandedId,setExpandedId]=useState<string|null>(null);

  const processDocuments=async()=>{
    setBusy("process");setProcessError(null);setSummary(null);
    try{
      const result=await commands.process_matter_documents({matterId}) as DocumentIntakeSummary;
      setSummary(result);
      reload();
    }catch(e){ setProcessError(String(e)); }
    finally{ setBusy(null); }
  };
  const retry=async(documentId:string)=>{
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
    <div className="card-head"><div><span className="eyebrow">SOURCE GRAPH</span><h2>מסמכים</h2></div>
      <button className="btn primary" onClick={processDocuments} disabled={busy==="process"}>{busy==="process"?"סורק ומעבד...":"סרוק ועבד מסמכים"}</button>
    </div>
    <p className="quiet">פעולה אחת: איתור קבצים חדשים, גיבוב, חילוץ טקסט (כולל OCR לסרוקים), וסיווג אוטומטי. סיווג ידני אינו נדרס לעולם.</p>
    {processError && <p className="quiet">שגיאה: {processError}</p>}
    {summary && <div className="workspace-card" style={{marginTop:8,marginBottom:8}}>
      <strong>{summary.discovered} מסמכים · {summary.extracted} חולצו · {summary.ocred} OCR · {summary.alreadyComplete} כבר הושלמו
        {summary.unsupported>0 && ` · ${summary.unsupported} לא נתמכים`}
        {summary.failed>0 && ` · ${summary.failed} דורשים טיפול`}
      </strong>
    </div>}
    {loading && <p className="quiet">טוען מסמכים...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && documents?.length===0 && <p className="quiet">אין עדיין מסמכים בתיק זה. יש לוודא שהתיקייה משויכת ולסרוק.</p>}
    <div className="table documents-table">
      <div className="tr th"><span>קובץ</span><span>קטגוריה</span><span>מצב מקור</span><span>חילוץ</span><span>פעולות</span></div>
      {documents?.map(d=><div key={d.id}>
        <div className="tr">
          <span><b>{d.fileName}</b><small>{d.modifiedAt}{d.pageCount>0 && ` · ${d.pageCount} עמודים/בלוקים`}</small></span>
          <span>
            <select value={d.category} disabled={busy===d.id} onChange={e=>changeCategory(d.id,e.target.value)}>
              {CATEGORIES.map(c=><option key={c} value={c}>{CATEGORY_LABELS[c]??c}</option>)}
            </select>
            <StatusBadge tone={d.categorySource==="manual"?"ok":"neutral"}>{d.categorySource==="manual"?"ידני":"אוטומטי"}</StatusBadge>
          </span>
          <span><StatusBadge tone={d.sourceState==="local"?"ok":"warn"}>{d.sourceState}</StatusBadge></span>
          <span>
            {d.extractionState==="complete" && <StatusBadge tone="ok">{d.extractionMethod==="tesseract"?"הושלם (OCR)":"הושלם"}</StatusBadge>}
            {d.extractionState==="failed" && <div>
              <StatusBadge tone="risk">{errorLabel(d.lastErrorCode)}</StatusBadge>
              <button className="btn secondary" onClick={()=>retry(d.id)} disabled={busy===d.id}>{busy===d.id?"מנסה שוב...":"נסה שוב"}</button>
            </div>}
            {(d.extractionState==="not_started"||d.extractionState==="pending"||d.extractionState==="blocked") &&
              <StatusBadge tone="warn">{d.extractionState}</StatusBadge>}
          </span>
          <span>
            {d.occurrenceId && <>
              <button className="btn secondary" onClick={()=>openFile(d.occurrenceId!)} disabled={busy===d.occurrenceId}>פתח</button>
              <button className="btn secondary" onClick={()=>revealFile(d.occurrenceId!)} disabled={busy===d.occurrenceId}>הצג בתיקייה</button>
            </>}
            {d.currentVersionId && <button className="btn secondary" onClick={()=>setExpandedId(expandedId===d.id?null:d.id)}>
              {expandedId===d.id?"סגור פרטים":"פרטי חילוץ"}
            </button>}
          </span>
        </div>
        {expandedId===d.id && d.currentVersionId && <DocumentDetail documentVersionId={d.currentVersionId}/>}
      </div>)}
    </div>
  </section>;
}
