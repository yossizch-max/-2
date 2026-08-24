import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { VerifiedFact } from "../types";

export function FactsAITab({matterId}:{matterId:string}) {
  const {data:facts,loading,error,reload}=useCommand(
    ()=>commands.list_verified_facts({matterId}) as Promise<VerifiedFact[]>, [matterId]
  );

  const invalidate=async(factId:string)=>{
    await commands.invalidate_fact({factId});
    reload();
  };

  return <div className="grid-2">
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">VERIFIED LEDGER</span><h2>עובדות מאומתות</h2></div><StatusBadge tone="ok">Human approved</StatusBadge></div>
      {loading && <p className="quiet">טוען עובדות...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && facts?.length===0 && <p className="quiet">אין עדיין עובדות מאומתות בתיק זה.</p>}
      {facts?.map(f=><div className="fact-row" key={f.id}>
        <strong>{f.subject} · {f.predicate}</strong><p>{f.value}</p>
        <button className="source-link">פתח מקור · {f.sourceLabel}</button>
        <button className="source-link" onClick={()=>invalidate(f.id)}>בטל תוקף</button>
      </div>)}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span><h2>הצעות לבדיקה</h2>
      <p className="quiet">הצעות AI מוצגות לפי הרצה (AI Run) ספציפית. הפעילו יכולת AI מתיק זה כדי לראות הצעות ממתינות לבדיקה כאן.</p>
      <p className="quiet">AI אינו כותב approved state ישירות.</p>
    </section>
  </div>;
}
