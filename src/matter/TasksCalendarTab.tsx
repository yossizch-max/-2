import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { Task, Deadline } from "../types";

export function TasksCalendarTab({matterId}:{matterId:string}) {
  const {data:tasks,loading:tasksLoading,reload:reloadTasks}=useCommand(
    ()=>commands.list_tasks({matterId}) as Promise<Task[]>, [matterId]
  );
  const {data:deadlines,loading:deadlinesLoading}=useCommand(
    ()=>commands.list_deadlines({matterId}) as Promise<Deadline[]>, [matterId]
  );
  const [title,setTitle]=useState("");
  const [busy,setBusy]=useState(false);

  const addTask=async()=>{
    if(!title.trim())return;
    setBusy(true);
    try{ await commands.create_task({matterId,title}); setTitle(""); reloadTasks(); }
    finally{ setBusy(false); }
  };
  const complete=async(taskId:string)=>{ await commands.complete_task({taskId}); reloadTasks(); };

  return <div className="grid-2">
    <section className="workspace-card">
      <h2>משימות</h2>
      <div className="header-actions">
        <input value={title} onChange={e=>setTitle(e.target.value)} placeholder="משימה חדשה..." onKeyDown={e=>e.key==="Enter"&&addTask()}/>
        <button className="btn secondary" onClick={addTask} disabled={busy||!title.trim()}>הוסף</button>
      </div>
      {tasksLoading && <p className="quiet">טוען...</p>}
      {!tasksLoading && tasks?.length===0 && <p className="quiet">אין משימות.</p>}
      {tasks?.map(t=><div className="timeline-row" key={t.id}>
        <b>{t.dueAt??"—"}</b>
        <div><strong>{t.title}</strong><small>{t.riskClass}</small></div>
        {t.status==="open" && <button className="btn secondary" onClick={()=>complete(t.id)}>סמן כבוצע</button>}
      </div>)}
    </section>
    <section className="workspace-card">
      <h2>מועדים</h2>
      {deadlinesLoading && <p className="quiet">טוען...</p>}
      {!deadlinesLoading && deadlines?.length===0 && <p className="quiet">אין מועדים.</p>}
      {deadlines?.map(d=><div className="timeline-row" key={d.id}><b>{d.dueAt}</b><div><strong>{d.action}</strong><small>{d.sourceLabel} · {d.state}</small></div></div>)}
    </section>
  </div>;
}
