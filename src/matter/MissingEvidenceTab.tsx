import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { REQUIREMENT_KEYS, REQUIREMENT_STATUSES, REQUIREMENT_PRIORITIES, type MatterRequirement } from "../types";

export function MissingEvidenceTab({matterId}:{matterId:string}) {
  const {data:requirements,reload}=useCommand(
    ()=>commands.list_matter_requirements({matterId}) as Promise<MatterRequirement[]>, [matterId]
  );
  const [busyKey,setBusyKey]=useState<string|null>(null);
  const [notesDraft,setNotesDraft]=useState<Record<string,string>>({});

  const keyLabel=(v:string)=>REQUIREMENT_KEYS.find(k=>k.value===v)?.label??v;
  const priorityLabel=(v:string)=>REQUIREMENT_PRIORITIES.find(p=>p.value===v)?.label??v;

  const setStatus=async(key:string,status:string,notes:string|null)=>{
    setBusyKey(key);
    try{
      await commands.update_matter_requirement({matterId,requirementKey:key,status,notes:notes||undefined});
      reload();
    }finally{setBusyKey(null);}
  };

  return <div className="matter-tab">
    <section className="workspace-card">
      <h2>ראיות חסרות</h2>
      <p className="quiet">רשימת המסמכים היא המלצה תפעולית של המשרד, לא קביעה משפטית - ניתן לשנות כל פריט בכל עת.</p>
      {requirements?.map(r=><div className="mini-row" key={r.requirementKey} style={{gridTemplateColumns:"1fr auto",alignItems:"center",display:"grid",gap:"10px"}}>
        <div style={{display:"grid",gap:"6px"}}>
          <strong>{keyLabel(r.requirementKey)}</strong>
          <small className="quiet">{priorityLabel(r.priority)}</small>
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
    </section>
  </div>;
}
