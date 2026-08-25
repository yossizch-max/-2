import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { CASE_TYPES, type Matter } from "../types";

export function MattersPage({onOpen}:{onOpen:(id:string)=>void}) {
  const {data:matters,loading,error,reload}=useCommand(
    ()=>commands.list_matters() as Promise<Matter[]>, []
  );
  const [creating,setCreating]=useState(false);
  const [title,setTitle]=useState("");
  const [internalNumber,setInternalNumber]=useState("");
  const [matterType,setMatterType]=useState("generic_civil");
  const [busy,setBusy]=useState(false);
  const [createError,setCreateError]=useState<string|null>(null);

  const submit=async()=>{
    if(!title.trim())return;
    setBusy(true);setCreateError(null);
    try{
      const res=await commands.create_matter({title,internalNumber:internalNumber||undefined,matterType}) as {id:string};
      setCreating(false);setTitle("");setInternalNumber("");setMatterType("generic_civil");
      reload();
      onOpen(res.id);
    }catch(e){setCreateError(String(e));}
    finally{setBusy(false);}
  };

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">MATTERS</span><h1>תיקים</h1><p>התיקיות נשארות מקור האמת. TAHRIR מוסיפה ידע, פעולה ומקור.</p></div><button className="btn primary" onClick={()=>setCreating(true)}>תיק חדש</button></div>
    {loading && <p className="quiet">טוען תיקים...</p>}
    {error && <p className="quiet">שגיאה בטעינת תיקים: {error}</p>}
    {!loading && !error && matters?.length===0 && <p className="quiet">אין עדיין תיקים. לחצו על "תיק חדש" כדי להתחיל.</p>}
    <div className="matter-list">{matters?.map(m=><button className="matter-row" onClick={()=>onOpen(m.id)} key={m.id}>
      <div className="matter-avatar">{m.title.slice(0,1)}</div>
      <div className="matter-main"><span className="eyebrow">{m.internalNumber}</span><strong>{m.title}</strong><small>{m.documentCount} מסמכים · {m.verifiedFactCount} עובדות · {m.pendingReviewCount} לבדיקה</small></div>
      <span className="chev">←</span>
    </button>)}</div>
    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>תיק חדש</h2>
        <label>כותרת התיק<input autoFocus value={title} onChange={e=>setTitle(e.target.value)} placeholder="לדוגמה: כהן נ׳ כלל חברה לביטוח"/></label>
        <label>מספר פנימי (אופציונלי)<input value={internalNumber} onChange={e=>setInternalNumber(e.target.value)}/></label>
        <label>סוג תיק<select value={matterType} onChange={e=>setMatterType(e.target.value)}>
          {CASE_TYPES.map(t=><option key={t.value} value={t.value}>{t.label}</option>)}
        </select></label>
        {createError && <p className="quiet">{createError}</p>}
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy||!title.trim()}>{busy?"יוצר...":"צור תיק"}</button>
        </div>
      </div>
    </div>}
  </div>;
}
