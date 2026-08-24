import { deadlines, tasks } from "../lib/demo";
export function CalendarPage() {
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">CALENDAR</span><h1>יומן ומשימות</h1><p>מועד משפטי הוא אובייקט עם מקור וכלל, לא תא תאריך.</p></div></div>
    <section className="workspace-card"><h2>מועדים מחייבים</h2>{deadlines.map(d=><div className="timeline-row" key={d.id}><b>{d.dueAt}</b><div><strong>{d.action}</strong><small>{d.sourceLabel} · {d.ruleLabel}</small></div></div>)}</section>
    <section className="workspace-card"><h2>משימות</h2>{tasks.map(t=><div className="timeline-row" key={t.id}><b>{t.dueAt}</b><div><strong>{t.title}</strong><small>{t.riskClass}</small></div></div>)}</section>
  </div>;
}
