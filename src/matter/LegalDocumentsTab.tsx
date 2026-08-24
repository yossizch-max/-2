import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { LegalDocument } from "../types";

export function LegalDocumentsTab({matterId}:{matterId:string}) {
  const {data:legalDocuments,loading,error,reload}=useCommand(
    ()=>commands.list_legal_documents({matterId}) as Promise<LegalDocument[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [title,setTitle]=useState("");
  const [kind,setKind]=useState("demand");
  const [busy,setBusy]=useState(false);

  const submit=async()=>{
    if(!title.trim())return;
    setBusy(true);
    try{ await commands.save_legal_document_draft({matterId,title,kind}); setCreating(false);setTitle(""); reload(); }
    finally{ setBusy(false); }
  };

  return <div>
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">LEGAL DOCUMENTS</span><h2>מסמכים משפטיים</h2></div><button className="btn primary" onClick={()=>setCreating(true)}>טיוטה חדשה</button></div>
      {loading && <p className="quiet">טוען...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && legalDocuments?.length===0 && <p className="quiet">אין עדיין מסמכים משפטיים בתיק זה.</p>}
      {legalDocuments?.map(d=><div className="legal-card" key={d.id}><div><strong>{d.title}</strong><small>{d.kind} · {d.status}</small></div></div>)}
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
  </div>;
}
