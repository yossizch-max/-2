import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { Authority } from "../types";

export function AuthoritiesTab({matterId}:{matterId:string}) {
  const {data:authorities,loading,error,reload}=useCommand(
    ()=>commands.list_authorities({matterId}) as Promise<Authority[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [citation,setCitation]=useState("");
  const [title,setTitle]=useState("");
  const [busy,setBusy]=useState<string|null>(null);

  const submit=async()=>{
    if(!citation.trim()||!title.trim())return;
    setBusy("create");
    try{ await commands.save_authority({matterId,citation,title}); setCreating(false);setCitation("");setTitle(""); reload(); }
    finally{ setBusy(null); }
  };
  const verify=async(authorityId:string)=>{
    setBusy(authorityId);
    try{ await commands.verify_authority({matterId,authorityId}); reload(); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">RESEARCH</span><h2>אסמכתאות</h2></div><button className="btn primary" onClick={()=>setCreating(true)}>הוסף מקור</button></div>
    <p className="quiet">אין scraping מנבו/תקדין. עורך הדין שומר מקור כדין ורק לאחר בדיקה הוא הופך Verified Authority.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && authorities?.length===0 && <p className="quiet">אין עדיין אסמכתאות בתיק זה.</p>}
    {authorities?.map(a=><div className="authority-row" key={a.id}>
      <div><strong>{a.citation}</strong><small>{a.title}</small></div>
      {a.status==="draft"
        ? <button className="btn secondary" onClick={()=>verify(a.id)} disabled={busy===a.id}>{busy===a.id?"מאמת...":"אמת"}</button>
        : <StatusBadge tone="ok">{a.status}</StatusBadge>}
    </div>)}
    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>אסמכתא חדשה</h2>
        <label>ציטוט<input autoFocus value={citation} onChange={e=>setCitation(e.target.value)} placeholder="רע״א 1234/20"/></label>
        <label>כותרת<input value={title} onChange={e=>setTitle(e.target.value)}/></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy==="create"}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy==="create"||!citation.trim()||!title.trim()}>{busy==="create"?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}
