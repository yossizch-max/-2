import { deadlines, tasks } from "../lib/demo";
export function TasksCalendarTab() {
  return <div className="grid-2">
    <section className="workspace-card"><h2>משימות</h2>{tasks.map(t=><div className="timeline-row" key={t.id}><b>{t.dueAt}</b><div><strong>{t.title}</strong><small>{t.riskClass}</small></div></div>)}</section>
    <section className="workspace-card"><h2>מועדים</h2>{deadlines.map(d=><div className="timeline-row" key={d.id}><b>{d.dueAt}</b><div><strong>{d.action}</strong><small>{d.sourceLabel}</small></div></div>)}</section>
  </div>;
}
