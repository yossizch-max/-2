import { facts } from "../lib/demo";
import { StatusBadge } from "../components/StatusBadge";

export function FactsAITab() {
  return <div className="grid-2">
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">VERIFIED LEDGER</span><h2>עובדות מאומתות</h2></div><StatusBadge tone="ok">Human approved</StatusBadge></div>
      {facts.map(f=><div className="fact-row" key={f.id}><strong>{f.subject} · {f.predicate}</strong><p>{f.value}</p><button className="source-link">פתח מקור · {f.sourceLabel}</button></div>)}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span><h2>הצעות לבדיקה</h2>
      <div className="proposal"><strong>אירוע רפואי מוצע</strong><p>“אושפז במחלקה האורתופדית...”</p><button className="source-link">מקור מדויק</button><div className="proposal-actions"><button>דחה</button><button>תקן</button><button className="primary-lite">אשר כעובדה</button></div></div>
      <p className="quiet">AI אינו כותב approved state ישירות.</p>
    </section>
  </div>;
}
