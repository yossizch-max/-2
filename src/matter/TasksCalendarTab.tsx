import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { Task, Deadline } from "../types";

type WaitingFor = {id:string;matterId:string;partyLabel:string;itemLabel:string;followUpAt?:string|null;status:string};
type CalendarEvent = {id:string;matterId:string;title:string;startsAt:string;endsAt?:string|null;eventKind:string;status:string};

export function TasksCalendarTab({matterId}:{matterId:string}) {
  const {data:tasks,loading:tasksLoading,reload:reloadTasks}=useCommand(
    ()=>commands.list_tasks({matterId}) as Promise<Task[]>, [matterId]
  );
  const {data:deadlines,loading:deadlinesLoading,reload:reloadDeadlines}=useCommand(
    ()=>commands.list_deadlines({matterId}) as Promise<Deadline[]>, [matterId]
  );
  const {data:waiting,loading:waitingLoading,reload:reloadWaiting}=useCommand(
    ()=>commands.list_waiting_for({matterId}) as Promise<WaitingFor[]>, [matterId]
  );
  const {data:events,loading:eventsLoading,reload:reloadEvents}=useCommand(
    ()=>commands.list_calendar_items({matterId}) as Promise<CalendarEvent[]>, [matterId]
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

  const [action,setAction]=useState("");
  const [dueAt,setDueAt]=useState("");
  const [sourceRef,setSourceRef]=useState("");
  const [deadlineBusy,setDeadlineBusy]=useState(false);
  const addDeadline=async()=>{
    if(!action.trim()||!dueAt||!sourceRef.trim())return;
    setDeadlineBusy(true);
    try{ await commands.save_manual_deadline({matterId,action,dueAt,triggerSourceRef:sourceRef}); setAction("");setDueAt("");setSourceRef(""); reloadDeadlines(); }
    finally{ setDeadlineBusy(false); }
  };
  const commit=async(deadlineId:string)=>{ await commands.commit_deadline({deadlineId}); reloadDeadlines(); };

  const [partyLabel,setPartyLabel]=useState("");
  const [itemLabel,setItemLabel]=useState("");
  const [waitingBusy,setWaitingBusy]=useState<string|null>(null);
  const addWaiting=async()=>{
    if(!partyLabel.trim()||!itemLabel.trim())return;
    setWaitingBusy("new");
    try{ await commands.save_waiting_for({matterId,partyLabel,itemLabel}); setPartyLabel("");setItemLabel(""); reloadWaiting(); }
    finally{ setWaitingBusy(null); }
  };
  const closeWaiting=async(id:string)=>{
    setWaitingBusy(id);
    try{ await commands.close_waiting_for({waitingForId:id}); reloadWaiting(); }
    finally{ setWaitingBusy(null); }
  };

  const [eventTitle,setEventTitle]=useState("");
  const [startsAt,setStartsAt]=useState("");
  const [eventBusy,setEventBusy]=useState(false);
  const addEvent=async()=>{
    if(!eventTitle.trim()||!startsAt)return;
    setEventBusy(true);
    try{ await commands.create_calendar_item({matterId,title:eventTitle,startsAt}); setEventTitle("");setStartsAt(""); reloadEvents(); }
    finally{ setEventBusy(false); }
  };

  return <div>
    <div className="grid-2">
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
        <div className="header-actions">
          <input value={action} onChange={e=>setAction(e.target.value)} placeholder="פעולה (למשל: הגשת תגובה)"/>
          <input type="date" value={dueAt} onChange={e=>setDueAt(e.target.value)}/>
          <input value={sourceRef} onChange={e=>setSourceRef(e.target.value)} placeholder="מקור (למשל: החלטה 20.8, עמ׳ 2)"/>
          <button className="btn secondary" onClick={addDeadline} disabled={deadlineBusy||!action.trim()||!dueAt||!sourceRef.trim()}>הוסף מועד</button>
        </div>
        {deadlinesLoading && <p className="quiet">טוען...</p>}
        {!deadlinesLoading && deadlines?.length===0 && <p className="quiet">אין מועדים.</p>}
        {deadlines?.map(d=><div className="timeline-row" key={d.id}>
          <b>{d.dueAt}</b>
          <div><strong>{d.action}</strong><small>{d.sourceLabel} · {d.state}</small></div>
          {d.state==="draft" && <button className="btn secondary" onClick={()=>commit(d.id)}>אשר (Commit)</button>}
        </div>)}
      </section>
    </div>
    <section className="workspace-card">
      <h2>ממתין ל-</h2>
      <div className="header-actions">
        <input value={partyLabel} onChange={e=>setPartyLabel(e.target.value)} placeholder="ממתין ל... (למשל: בית חולים הדסה)"/>
        <input value={itemLabel} onChange={e=>setItemLabel(e.target.value)} placeholder="מה (למשל: תיק רפואי)"/>
        <button className="btn secondary" onClick={addWaiting} disabled={waitingBusy==="new"||!partyLabel.trim()||!itemLabel.trim()}>הוסף</button>
      </div>
      {waitingLoading && <p className="quiet">טוען...</p>}
      {!waitingLoading && waiting?.length===0 && <p className="quiet">אין פריטים ממתינים.</p>}
      {waiting?.map(w=><div className="timeline-row" key={w.id}>
        <div><strong>{w.partyLabel}</strong><small>{w.itemLabel}{w.followUpAt?` · מעקב: ${w.followUpAt}`:""}</small></div>
        {w.status==="open" && <button className="btn secondary" onClick={()=>closeWaiting(w.id)} disabled={waitingBusy===w.id}>סגור</button>}
      </div>)}
    </section>
    <section className="workspace-card">
      <h2>אירועי יומן</h2>
      <div className="header-actions">
        <input value={eventTitle} onChange={e=>setEventTitle(e.target.value)} placeholder="כותרת (למשל: דיון קדם משפט)"/>
        <input type="datetime-local" value={startsAt} onChange={e=>setStartsAt(e.target.value)}/>
        <button className="btn secondary" onClick={addEvent} disabled={eventBusy||!eventTitle.trim()||!startsAt}>הוסף אירוע</button>
      </div>
      {eventsLoading && <p className="quiet">טוען...</p>}
      {!eventsLoading && events?.length===0 && <p className="quiet">אין אירועים.</p>}
      {events?.map(e=><div className="timeline-row" key={e.id}>
        <b>{e.startsAt}</b>
        <div><strong>{e.title}</strong><small>{e.eventKind}</small></div>
      </div>)}
    </section>
  </div>;
}
