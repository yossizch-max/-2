import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { REQUIREMENT_KEYS, REQUIREMENT_STATUSES, REQUIREMENT_PRIORITIES, type MatterRequirement } from "../types";

// UX Milestone 1: missing evidence is never its own sub-tab inside עבודת התיק -
// it appears contextually instead, either as the full list on בית התיק or as a
// domain-filtered slice inside a domain's own evidence view. Both call sites share
// this one panel and the same backend read/update path; nothing here is duplicated.
export function RequirementsPanel({matterId,filterKeys,title="ראיות חסרות",emptyText}:{
  matterId:string; filterKeys?:string[]; title?:string; emptyText?:string;
}) {
  const {data:requirements,reload}=useCommand(
    ()=>commands.list_matter_requirements({matterId}) as Promise<MatterRequirement[]>, [matterId]
  );
  const [busyKey,setBusyKey]=useState<string|null>(null);
  const [notesDraft,setNotesDraft]=useState<Record<string,string>>({});

  const keyLabel=(v:string)=>REQUIREMENT_KEYS.find(k=>k.value===v)?.label??v;
  const priorityLabel=(v:string)=>REQUIREMENT_PRIORITIES.find(p=>p.value===v)?.label??v;
  const relevanceLabel=(r:MatterRequirement)=>
    r.relevance==="not_applicable" ? "לא רלוונטי" : (r.priority ? priorityLabel(r.priority) : "");

  const setStatus=async(key:string,status:string,notes:string|null)=>{
    setBusyKey(key);
    try{
      await commands.update_matter_requirement({matterId,requirementKey:key,status,notes:notes||undefined});
      reload();
    }finally{setBusyKey(null);}
  };

  const visible=filterKeys ? requirements?.filter(r=>filterKeys.includes(r.requirementKey)) : requirements;

  return <section className="workspace-card">
    <h2>{title}</h2>
    <p className="quiet">רשימת המסמכים היא המלצה תפעולית של המשרד, לא קביעה משפטית - ניתן לשנות כל פריט בכל עת.</p>
    {visible?.length===0 && <p className="quiet">{emptyText ?? "אין פריטים רלוונטיים כרגע."}</p>}
    {visible?.map(r=><div className="mini-row" key={r.requirementKey} style={{gridTemplateColumns:"1fr auto",alignItems:"center",display:"grid",gap:"10px"}}>
      <div style={{display:"grid",gap:"6px"}}>
        <strong>{keyLabel(r.requirementKey)}</strong>
        <small className="quiet">{relevanceLabel(r)}</small>
        <input
          placeholder="הערות"
          defaultValue={r.notes??""}
          onChange={e=>setNotesDraft(d=>({...d,[r.requirementKey]:e.target.value}))}
          onBlur={()=>{const n=notesDraft[r.requirementKey];if(n!==undefined)setStatus(r.requirementKey,r.status,n);}}
        />
      </div>
      <select
        value={r.status}
        disabled={busyKey===r.requirementKey}
        onChange={e=>setStatus(r.requirementKey,e.target.value,notesDraft[r.requirementKey]??r.notes??null)}
      >
        {REQUIREMENT_STATUSES.map(s=><option key={s.value} value={s.value}>{s.label}</option>)}
      </select>
    </div>)}
  </section>;
}

export function MissingEvidenceTab({matterId}:{matterId:string}) {
  return <div className="matter-tab"><RequirementsPanel matterId={matterId}/></div>;
}
