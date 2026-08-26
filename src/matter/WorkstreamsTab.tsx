import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { WORKSTREAM_KINDS, WORKSTREAM_STATUSES, type Workstream } from "../types";

export function WorkstreamsTab({matterId}:{matterId:string}) {
  const {data:workstreams,reload}=useCommand(
    ()=>commands.list_matter_workstreams({matterId}) as Promise<Workstream[]>, [matterId]
  );
  const [busyKind,setBusyKind]=useState<string|null>(null);
  const [notesDraft,setNotesDraft]=useState<Record<string,string>>({});

  const kindLabel=(v:string)=>WORKSTREAM_KINDS.find(k=>k.value===v)?.label??v;

  const setStatus=async(kind:string,status:string,notes:string|null)=>{
    setBusyKind(kind);
    try{
      await commands.update_matter_workstream({matterId,kind,status,notes:notes||undefined});
      reload();
    }finally{setBusyKind(null);}
  };

  return <div className="matter-tab">
    <section className="workspace-card">
      <h2>מסלולי עבודה</h2>
      <p className="quiet">ברירות המחדל נגזרות מסוג התיק ואינן קביעה משפטית - ניתן לשנות כל מסלול בכל עת.</p>
      {workstreams?.map(w=><div className="mini-row" key={w.kind} style={{gridTemplateColumns:"1fr auto",alignItems:"center",display:"grid",gap:"10px"}}>
        <div style={{display:"grid",gap:"6px"}}>
          <strong>{kindLabel(w.kind)}</strong>
          <input
            placeholder="הערות"
            defaultValue={w.notes??""}
            onChange={e=>setNotesDraft(d=>({...d,[w.kind]:e.target.value}))}
            onBlur={()=>{const n=notesDraft[w.kind];if(n!==undefined)setStatus(w.kind,w.status,n);}}
          />
        </div>
        <select
          value={w.status}
          disabled={busyKind===w.kind}
          onChange={e=>setStatus(w.kind,e.target.value,notesDraft[w.kind]??w.notes??null)}
        >
          {WORKSTREAM_STATUSES.map(s=><option key={s.value} value={s.value}>{s.label}</option>)}
        </select>
      </div>)}
    </section>
  </div>;
}
