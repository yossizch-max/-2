import { documents, tasks, deadlines } from "../lib/demo";
import type { Matter } from "../types";

export function OverviewTab({matter}:{matter:Matter}) {
  return <div className="matter-tab">
    <div className="kpi-grid">
      <button><span>מסמכים</span><strong>{matter.documentCount}</strong><small>מקור פתיח בלחיצה</small></button>
      <button><span>עובדות</span><strong>{matter.verifiedFactCount}</strong><small>{matter.pendingReviewCount} לבדיקה</small></button>
      <button><span>מועד קרוב</span><strong>{deadlines[0]?.dueAt ?? "—"}</strong><small>מחייב רק לאחר commit</small></button>
      <button><span>שלב</span><strong>{matter.workflowStage}</strong><small>המעבר מאושר בידי המשתמש</small></button>
    </div>
    <div className="grid-2">
      <section className="workspace-card"><h2>הפעולה הבאה</h2><div className="next-action"><strong>{tasks[0]?.title}</strong><p>ממתין למעקב. ניתן לפתוח את המקור מתוך Timeline.</p><button className="btn primary">פתח פעולה</button></div></section>
      <section className="workspace-card"><h2>מסמכים אחרונים</h2>{documents.slice(0,3).map(d=><button className="mini-row" key={d.id}><span>{d.fileName}</span><small>{d.category} · {d.extractionState}</small></button>)}</section>
    </div>
  </div>;
}
