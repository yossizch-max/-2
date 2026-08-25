import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { Authority, AuthorityPassage, DocumentPage, DocumentRow } from "../types";

function PassageManager({matterId,authority,onChanged}:{matterId:string;authority:Authority;onChanged:()=>void}) {
  const {data:passages,loading,error,reload}=useCommand(
    ()=>commands.list_authority_passages({matterId,authorityId:authority.id}) as Promise<AuthorityPassage[]>,
    [authority.id]
  );
  const {data:pages}=useCommand(
    ()=>authority.sourceDocumentVersionId
      ? commands.get_document_pages({documentVersionId:authority.sourceDocumentVersionId}) as Promise<DocumentPage[]>
      : Promise.resolve([]),
    [authority.sourceDocumentVersionId]
  );

  const [pageId,setPageId]=useState("");
  const [passageText,setPassageText]=useState("");
  const [issueTag,setIssueTag]=useState("");
  const [busy,setBusy]=useState<string|null>(null);
  const [formError,setFormError]=useState<string|null>(null);
  const selectedPage=pages?.find(p=>p.id===pageId);

  const addPassage=async()=>{
    if(!pageId||!passageText.trim())return;
    setBusy("add");setFormError(null);
    try{
      await commands.add_authority_passage({
        matterId,authorityId:authority.id,sourcePageId:pageId,
        passageText,issueTag:issueTag.trim()||undefined
      });
      setPassageText("");setIssueTag("");
      reload();onChanged();
    }catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };
  const approvePassage=async(passageId:string)=>{
    setBusy(passageId);setFormError(null);
    try{ await commands.approve_authority_passage({matterId,authorityId:authority.id,passageId}); reload();onChanged(); }
    catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };

  return <div className="workspace-card" style={{marginTop:10}}>
    <h3>קטעים מצוטטים</h3>
    <p className="quiet">אימות האסמכתא דורש לפחות קטע מאושר אחד, שנבדק מילה-במילה מול טקסט המקור בתיק.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {passages?.length===0 && <p className="quiet">אין עדיין קטעים.</p>}
    {passages?.map(p=><div className="authority-row" key={p.id}>
      <div>
        <strong>{p.issueTag||"קטע"}</strong>
        <small>{p.fileName??"—"}{p.page?` · עמוד ${p.page}`:""} · {p.passageText}</small>
      </div>
      {p.approved
        ? <StatusBadge tone="ok">מאושר</StatusBadge>
        : <button className="btn secondary" disabled={busy===p.id} onClick={()=>approvePassage(p.id)}>{busy===p.id?"מאשר...":"אשר קטע"}</button>}
    </div>)}

    {!authority.sourceDocumentVersionId && <p className="quiet">יש לשייך מסמך מקור לאסמכתא לפני הוספת קטעים.</p>}
    {authority.sourceDocumentVersionId && <div style={{marginTop:12}}>
      <label>עמוד מקור
        <select value={pageId} onChange={e=>setPageId(e.target.value)}>
          <option value="">בחר עמוד...</option>
          {pages?.map(p=><option key={p.id} value={p.id}>{p.pageNumber?`עמוד ${p.pageNumber}`:p.id}</option>)}
        </select>
      </label>
      {selectedPage && <blockquote className="source-excerpt"><p>{selectedPage.text}</p></blockquote>}
      <label>ציטוט מדויק מהעמוד (חייב להופיע מילה-במילה)
        <textarea value={passageText} onChange={e=>setPassageText(e.target.value)} rows={3}/>
      </label>
      <label>נושא (אופציונלי)<input value={issueTag} onChange={e=>setIssueTag(e.target.value)} placeholder="למשל: אשם תורם"/></label>
      <button className="btn secondary" onClick={addPassage} disabled={busy==="add"||!pageId||!passageText.trim()}>
        {busy==="add"?"מוסיף...":"הוסף קטע"}
      </button>
    </div>}
  </div>;
}

export function AuthoritiesTab({matterId}:{matterId:string}) {
  const {data:authorities,loading,error,reload}=useCommand(
    ()=>commands.list_authorities({matterId}) as Promise<Authority[]>, [matterId]
  );
  const {data:documents}=useCommand(
    ()=>commands.list_documents({matterId}) as Promise<DocumentRow[]>, [matterId]
  );
  const sourceableDocuments=documents?.filter(d=>d.currentVersionId)??[];
  const [creating,setCreating]=useState(false);
  const [citation,setCitation]=useState("");
  const [title,setTitle]=useState("");
  const [sourceVersionId,setSourceVersionId]=useState("");
  const [busy,setBusy]=useState<string|null>(null);
  const [formError,setFormError]=useState<string|null>(null);
  const [managingId,setManagingId]=useState<string|null>(null);

  const submit=async()=>{
    if(!citation.trim()||!title.trim())return;
    setBusy("create");
    try{
      await commands.save_authority({matterId,citation,title,sourceDocumentVersionId:sourceVersionId||undefined});
      setCreating(false);setCitation("");setTitle("");setSourceVersionId("");
      reload();
    }finally{ setBusy(null); }
  };
  const verify=async(authorityId:string)=>{
    setBusy(authorityId);setFormError(null);
    try{ await commands.verify_authority({matterId,authorityId}); reload(); }
    catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">RESEARCH</span><h2>אסמכתאות</h2></div><button className="btn primary" onClick={()=>setCreating(true)}>הוסף מקור</button></div>
    <p className="quiet">אין scraping מנבו/תקדין. עורך הדין שומר מקור כדין, ומצטט ממנו קטע מאושר, ורק אז ניתן לאמת את האסמכתא.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {!loading && !error && authorities?.length===0 && <p className="quiet">אין עדיין אסמכתאות בתיק זה.</p>}
    {authorities?.map(a=><div key={a.id}>
      <div className="authority-row">
        <div><strong>{a.citation}</strong><small>{a.title} · {a.approvedPassageCount} קטעים מאושרים</small></div>
        <div className="header-actions">
          {a.status==="draft" && <button className="btn secondary" onClick={()=>setManagingId(managingId===a.id?null:a.id)}>
            {managingId===a.id?"סגור קטעים":"נהל קטעים"}
          </button>}
          {a.status==="draft"
            ? <button className="btn secondary" onClick={()=>verify(a.id)} disabled={busy===a.id}>{busy===a.id?"מאמת...":"אמת"}</button>
            : <StatusBadge tone="ok">{a.status}</StatusBadge>}
        </div>
      </div>
      {managingId===a.id && <PassageManager matterId={matterId} authority={a} onChanged={reload}/>}
    </div>)}
    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>אסמכתא חדשה</h2>
        <label>ציטוט<input autoFocus value={citation} onChange={e=>setCitation(e.target.value)} placeholder="רע״א 1234/20"/></label>
        <label>כותרת<input value={title} onChange={e=>setTitle(e.target.value)}/></label>
        <label>מסמך מקור בתיק (נדרש לאימות)
          <select value={sourceVersionId} onChange={e=>setSourceVersionId(e.target.value)}>
            <option value="">ללא (לא ניתן יהיה לאמת)</option>
            {sourceableDocuments.map(d=><option key={d.currentVersionId} value={d.currentVersionId??""}>{d.fileName}</option>)}
          </select>
        </label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy==="create"}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy==="create"||!citation.trim()||!title.trim()}>{busy==="create"?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}
