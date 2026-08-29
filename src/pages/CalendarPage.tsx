import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { Matter, Task, Deadline } from "../types";

async function loadCalendar(){
  const matters=(await commands.list_matters() as Matter[]).filter(m=>m.status==="active");
  const perMatter=await Promise.all(matters.map(async matter=>{
    const [deadlines,tasks]=await Promise.all([
      commands.list_deadlines({matterId:matter.id}) as Promise<Deadline[]>,
      commands.list_tasks({matterId:matter.id}) as Promise<Task[]>,
    ]);
    return {deadlines,tasks:tasks.filter(t=>t.status==="open")};
  }));
  return {
    deadlines:perMatter.flatMap(x=>x.deadlines).filter(d=>d.state==="committed").sort((a,b)=>a.dueAt.localeCompare(b.dueAt)),
    tasks:perMatter.flatMap(x=>x.tasks).sort((a,b)=>(a.dueAt??"").localeCompare(b.dueAt??"")),
  };
}

export function CalendarPage() {
  const {data,loading,error}=useCommand(loadCalendar,[]);
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">TASKS &amp; CALENDAR</span><h1>משימות ויומן</h1><p>מועד משפטי הוא אובייקט עם מקור וכלל, לא תא תאריך.</p></div></div>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    <section className="workspace-card"><h2>מועדים מחייבים</h2>
      {!loading && data?.deadlines.length===0 && <p className="quiet">אין מועדים.</p>}
      {data?.deadlines.map(d=><div className="timeline-row" key={d.id}><b>{d.dueAt}</b><div><strong>{d.action}</strong><small>{d.sourceLabel}{d.ruleLabel?` · ${d.ruleLabel}`:""}</small></div></div>)}
    </section>
    <section className="workspace-card"><h2>משימות</h2>
      {!loading && data?.tasks.length===0 && <p className="quiet">אין משימות פתוחות.</p>}
      {data?.tasks.map(t=><div className="timeline-row" key={t.id}><b>{t.dueAt??"—"}</b><div><strong>{t.title}</strong><small>{t.riskClass}</small></div></div>)}
    </section>
  </div>;
}
