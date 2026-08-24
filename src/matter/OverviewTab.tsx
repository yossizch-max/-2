import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { Matter, DocumentRow, Task, Deadline } from "../types";

export function OverviewTab({matter}:{matter:Matter}) {
  const {data:documents}=useCommand(
    ()=>commands.list_documents({matterId:matter.id}) as Promise<DocumentRow[]>, [matter.id]
  );
  const {data:tasks}=useCommand(
    ()=>commands.list_tasks({matterId:matter.id}) as Promise<Task[]>, [matter.id]
  );
  const {data:deadlines}=useCommand(
    ()=>commands.list_deadlines({matterId:matter.id}) as Promise<Deadline[]>, [matter.id]
  );
  const nextTask=tasks?.find(t=>t.status==="open");
  const nextDeadline=deadlines?.[0];

  return <div className="matter-tab">
    <div className="kpi-grid">
      <button><span>מסמכים</span><strong>{matter.documentCount}</strong><small>מקור פתיח בלחיצה</small></button>
      <button><span>עובדות</span><strong>{matter.verifiedFactCount}</strong><small>{matter.pendingReviewCount} לבדיקה</small></button>
      <button><span>מועד קרוב</span><strong>{nextDeadline?.dueAt ?? "—"}</strong><small>מחייב רק לאחר commit</small></button>
      <button><span>שלב</span><strong>{matter.workflowStage}</strong><small>המעבר מאושר בידי המשתמש</small></button>
    </div>
    <div className="grid-2">
      <section className="workspace-card"><h2>הפעולה הבאה</h2>
        {nextTask
          ? <div className="next-action"><strong>{nextTask.title}</strong><p>{nextTask.dueAt?`יעד: ${nextTask.dueAt}`:"אין יעד"} · {nextTask.riskClass}</p></div>
          : <p className="quiet">אין משימות פתוחות.</p>}
      </section>
      <section className="workspace-card"><h2>מסמכים אחרונים</h2>
        {documents?.length
          ? documents.slice(0,3).map(d=><button className="mini-row" key={d.id}><span>{d.fileName}</span><small>{d.category} · {d.extractionState}</small></button>)
          : <p className="quiet">אין עדיין מסמכים. סרקו את תיקיית התיק בלשונית מסמכים.</p>}
      </section>
    </div>
  </div>;
}
