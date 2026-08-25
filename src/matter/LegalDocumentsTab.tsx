import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { LegalDocument, LegalDocumentParagraph, LegalDocumentVersionDetail } from "../types";

export function LegalDocumentsTab({matterId}:{matterId:string}) {
  const {data:legalDocuments,loading,error,reload}=useCommand(
    ()=>commands.list_legal_documents({matterId}) as Promise<LegalDocument[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [title,setTitle]=useState("");
  const [kind,setKind]=useState("demand");
  const [busy,setBusy]=useState(false);
  const [versioning,setVersioning]=useState<string|null>(null);
  const [editingDoc,setEditingDoc]=useState<LegalDocument|null>(null);

  const submit=async()=>{
    if(!title.trim())return;
    setBusy(true);
    try{ await commands.save_legal_document_draft({matterId,title,kind}); setCreating(false);setTitle(""); reload(); }
    finally{ setBusy(false); }
  };

  const newVersion=async(legalDocumentId:string)=>{
    setVersioning(legalDocumentId);
    try{ await commands.create_legal_document_version({matterId,legalDocumentId}); reload(); }
    finally{ setVersioning(null); }
  };

  return <div>
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">LEGAL DOCUMENTS</span><h2>מסמכים משפטיים</h2></div><button className="btn primary" onClick={()=>setCreating(true)}>טיוטה חדשה</button></div>
      {loading && <p className="quiet">טוען...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && legalDocuments?.length===0 && <p className="quiet">אין עדיין מסמכים משפטיים בתיק זה.</p>}
      {legalDocuments?.map(d=><div
        className="legal-card" key={d.id} role="button" tabIndex={0}
        onClick={()=>setEditingDoc(d)}
        onKeyDown={(e)=>{if(e.key==="Enter"||e.key===" "){e.preventDefault();setEditingDoc(d);}}}
      >
        <div><strong>{d.title}</strong><small>{d.kind} · {d.status}</small></div>
        {d.status==="approved" && <button
          className="btn secondary"
          onClick={(e)=>{e.stopPropagation();if(versioning!==d.id)newVersion(d.id);}}
        >{versioning===d.id?"יוצר גרסה...":"צור גרסה חדשה"}</button>}
      </div>)}
    </section>
    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>טיוטת מסמך משפטי חדשה</h2>
        <label>כותרת<input autoFocus value={title} onChange={e=>setTitle(e.target.value)}/></label>
        <label>סוג<select value={kind} onChange={e=>setKind(e.target.value)}>
          <option value="demand">מכתב דרישה</option>
          <option value="claim">כתב תביעה</option>
          <option value="response">כתב הגנה/תגובה</option>
        </select></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy||!title.trim()}>{busy?"יוצר...":"צור טיוטה"}</button>
        </div>
      </div>
    </div>}
    {editingDoc && <LegalDocumentEditor
      matterId={matterId} doc={editingDoc}
      onClose={()=>setEditingDoc(null)}
      onChanged={reload}
    />}
  </div>;
}

function LegalDocumentEditor({matterId,doc,onClose,onChanged}:{
  matterId:string; doc:LegalDocument; onClose:()=>void; onChanged:()=>void;
}) {
  const versionId=doc.currentVersionId??"";
  const {data:version,loading,error,reload}=useCommand(
    ()=>commands.get_legal_document_version({matterId,versionId}) as Promise<LegalDocumentVersionDetail>,
    [matterId,versionId]
  );
  const [busy,setBusy]=useState(false);
  const [actionError,setActionError]=useState<string|null>(null);
  const [editingParagraphId,setEditingParagraphId]=useState<string|null>(null);
  const [draftText,setDraftText]=useState("");
  const [addingToSection,setAddingToSection]=useState<string|null>(null);
  const [newParagraphText,setNewParagraphText]=useState("");

  const isDraft=version?.status==="draft";

  const run=async(fn:()=>Promise<unknown>)=>{
    setBusy(true);setActionError(null);
    try{ await fn(); reload(); onChanged(); }
    catch(e){ setActionError(String(e)); }
    finally{ setBusy(false); }
  };

  const fillFacts=()=>run(()=>commands.fill_legal_document_facts({matterId,versionId}));
  const startEdit=(p:LegalDocumentParagraph)=>{ setEditingParagraphId(p.id); setDraftText(p.bodyText); };
  const saveEdit=(paragraphId:string)=>run(async()=>{
    await commands.update_legal_document_paragraph({matterId,versionId,paragraphId,bodyText:draftText});
    setEditingParagraphId(null);
  });
  const confirmParagraph=(paragraphId:string)=>run(()=>commands.confirm_legal_document_paragraph({matterId,versionId,paragraphId}));
  const deleteParagraph=(paragraphId:string)=>run(()=>commands.delete_legal_document_paragraph({matterId,versionId,paragraphId}));
  const addParagraph=(sectionId:string)=>run(async()=>{
    if(!newParagraphText.trim())return;
    await commands.add_legal_document_paragraph({matterId,versionId,sectionId,bodyText:newParagraphText});
    setAddingToSection(null); setNewParagraphText("");
  });

  return <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)onClose();}}>
    <div className="workspace-card" style={{width:"min(760px,94vw)",maxHeight:"88vh",overflowY:"auto"}}>
      <div className="card-head">
        <div><span className="eyebrow">LEGAL DOCUMENT</span><h2>{doc.title}</h2></div>
        {version && <span className={`status-badge ${version.status==="approved"?"ok":""}`}>{version.status}</span>}
        <button className="btn secondary" onClick={onClose}>סגור</button>
      </div>
      {loading && <p className="quiet">טוען...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {actionError && <p className="quiet">שגיאה: {actionError}</p>}
      {isDraft && <div className="header-actions">
        <button className="btn secondary" onClick={fillFacts} disabled={busy}>מלא עובדות מאומתות</button>
      </div>}
      {version && !isDraft && <p className="quiet">גרסה מאושרת וקבועה — לעריכה יש ליצור "גרסה חדשה" מרשימת המסמכים.</p>}
      <div className="legal-paper">
        {version?.sections.map(s=>
          <div key={s.id} className="paper-paragraph">
            <strong>{s.heading}</strong>
            {s.paragraphs.length===0 && <p className="quiet">אין עדיין פסקאות בפרק זה.</p>}
            {s.paragraphs.map(p=>
              <div key={p.id} style={{marginTop:10}}>
                {editingParagraphId===p.id
                  ? <>
                      <textarea value={draftText} onChange={e=>setDraftText(e.target.value)} rows={3} style={{width:"100%"}}/>
                      <div className="header-actions">
                        <button className="btn secondary" onClick={()=>setEditingParagraphId(null)} disabled={busy}>ביטול</button>
                        <button className="btn primary" onClick={()=>saveEdit(p.id)} disabled={busy}>שמור</button>
                      </div>
                    </>
                  : <>
                      <p>{p.bodyText}</p>
                      <div className="header-actions">
                        <span className={`status-badge ${p.provenanceState==="confirmed"?"ok":"warn"}`}>
                          {p.provenanceState==="confirmed"?"מאושר":"ממתין לאישור"}
                        </span>
                        {isDraft && <>
                          <button className="btn secondary" onClick={()=>startEdit(p)} disabled={busy}>ערוך</button>
                          {p.provenanceState!=="confirmed" &&
                            <button className="btn secondary" onClick={()=>confirmParagraph(p.id)} disabled={busy}>אשר</button>}
                          <button className="btn secondary" onClick={()=>deleteParagraph(p.id)} disabled={busy}>מחק</button>
                        </>}
                      </div>
                    </>}
              </div>
            )}
            {isDraft && (addingToSection===s.id
              ? <div style={{marginTop:10}}>
                  <textarea value={newParagraphText} onChange={e=>setNewParagraphText(e.target.value)} rows={3} style={{width:"100%"}} placeholder="טקסט הפסקה..."/>
                  <div className="header-actions">
                    <button className="btn secondary" onClick={()=>{setAddingToSection(null);setNewParagraphText("");}} disabled={busy}>ביטול</button>
                    <button className="btn primary" onClick={()=>addParagraph(s.id)} disabled={busy||!newParagraphText.trim()}>הוסף</button>
                  </div>
                </div>
              : <button className="btn secondary" style={{marginTop:10}} onClick={()=>setAddingToSection(s.id)}>הוסף פסקה</button>)}
          </div>
        )}
      </div>
    </div>
  </div>;
}
