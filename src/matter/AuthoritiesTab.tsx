import { authorities } from "../lib/demo";
import { StatusBadge } from "../components/StatusBadge";

export function AuthoritiesTab() {
  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">RESEARCH</span><h2>אסמכתאות</h2></div><button className="btn primary">הוסף מקור</button></div>
    <p className="quiet">אין scraping מנבו/תקדין. עורך הדין שומר מקור כדין ורק לאחר בדיקה הוא הופך Verified Authority.</p>
    {authorities.map(a=><div className="authority-row" key={a.id}><div><strong>{a.citation}</strong><small>{a.title}</small></div><StatusBadge tone="ok">{a.status}</StatusBadge></div>)}
  </section>;
}
