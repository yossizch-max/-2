import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { Matter } from "../types";
import { OverviewTab } from "../matter/OverviewTab";
import { DocumentsTab } from "../matter/DocumentsTab";
import { FactsAITab } from "../matter/FactsAITab";
import { DamageTab } from "../matter/DamageTab";
import { TasksCalendarTab } from "../matter/TasksCalendarTab";
import { LegalDocumentsTab } from "../matter/LegalDocumentsTab";
import { AuthoritiesTab } from "../matter/AuthoritiesTab";

type Tab="overview"|"documents"|"facts"|"damage"|"tasks"|"legal"|"research";
const STAGES=["intake","evidence_collection","treatment_and_records","negotiation","litigation","closed"];

function EditMatterModal({matter,onClose,onSaved}:{matter:Matter;onClose:()=>void;onSaved:()=>void}) {
  const [title,setTitle]=useState(matter.title);
  const [internalNumber,setInternalNumber]=useState(matter.internalNumber??"");
  const [externalNumber,setExternalNumber]=useState(matter.externalNumber??"");
  const [busy,setBusy]=useState(false);
  const [err,setErr]=useState<string|null>(null);

  const save=async()=>{
    setBusy(true);setErr(null);
    try{
      await commands.update_matter({matterId:matter.id,title,internalNumber:internalNumber||undefined,externalNumber:externalNumber||undefined});
      onSaved();onClose();
    }catch(e){setErr(String(e));}
    finally{setBusy(false);}
  };

  return <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)onClose();}}>
    <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
      <h2>עריכת פרטי תיק</h2>
      <label>כותרת<input autoFocus value={title} onChange={e=>setTitle(e.target.value)}/></label>
      <label>מספר פנימי<input value={internalNumber} onChange={e=>setInternalNumber(e.target.value)}/></label>
      <label>מספר חיצוני<input value={externalNumber} onChange={e=>setExternalNumber(e.target.value)}/></label>
      {err && <p className="quiet">{err}</p>}
      <div className="header-actions">
        <button className="btn secondary" onClick={onClose} disabled={busy}>ביטול</button>
        <button className="btn primary" onClick={save} disabled={busy||!title.trim()}>{busy?"שומר...":"שמור"}</button>
      </div>
    </div>
  </div>;
}

export function MatterWorkspace({matterId,onBack}:{matterId:string;onBack:()=>void}) {
  const {data:matter,loading,error,reload}=useCommand(
    ()=>commands.get_matter({matterId}) as Promise<Matter>, [matterId]
  );
  const [tab,setTab]=useState<Tab>("overview");
  const [editing,setEditing]=useState(false);
  const [stageBusy,setStageBusy]=useState(false);
  const tabs:Array<[Tab,string]>=[["overview","סקירה"],["documents","מסמכים"],["facts","עובדות ו־AI"],["damage","נזק"],["tasks","משימות ויומן"],["legal","מסמכים משפטיים"],["research","מחקר"]];

  const changeStage=async(stage:string)=>{
    setStageBusy(true);
    try{ await commands.set_matter_stage({matterId,stage}); reload(); }
    finally{ setStageBusy(false); }
  };

  if(loading) return <div className="page matter-page"><p className="quiet">טוען תיק...</p></div>;
  if(error || !matter) return <div className="page matter-page"><button className="back-link" onClick={onBack}>→ תיקים</button><p className="quiet">שגיאה בטעינת התיק: {error}</p></div>;

  return <div className="page matter-page">
    <div className="matter-header">
      <div><button className="back-link" onClick={onBack}>→ תיקים</button><span className="eyebrow">{matter.internalNumber} · {matter.matterType}</span><h1>{matter.title}</h1>
        <div className="meta-chips">
          <span>{matter.documentCount} מסמכים</span><span>{matter.verifiedFactCount} עובדות</span>
          <select value={matter.workflowStage} disabled={stageBusy} onChange={e=>changeStage(e.target.value)}>
            {STAGES.map(s=><option key={s} value={s}>{s}</option>)}
          </select>
        </div>
      </div>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setEditing(true)}>ערוך פרטים</button></div>
    </div>
    <nav className="matter-tabs" aria-label="אזורים בתיק">
      {tabs.map(([key,label])=><button key={key} aria-current={tab===key?"page":undefined} className={tab===key?"active":""} onClick={()=>setTab(key)}>{label}</button>)}
    </nav>
    <div className="tab-body">
      {tab==="overview"?<OverviewTab matter={matter}/>:tab==="documents"?<DocumentsTab matterId={matterId}/>:tab==="facts"?<FactsAITab matterId={matterId}/>:tab==="damage"?<DamageTab matterId={matterId}/>:tab==="tasks"?<TasksCalendarTab matterId={matterId}/>:tab==="legal"?<LegalDocumentsTab matterId={matterId}/>:<AuthoritiesTab matterId={matterId}/>}
    </div>
    {editing && <EditMatterModal matter={matter} onClose={()=>setEditing(false)} onSaved={reload}/>}
  </div>;
}
