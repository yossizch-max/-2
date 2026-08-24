import { calculations } from "../lib/demo";
const money=(c:number)=>(c/100).toLocaleString("he-IL",{style:"currency",currency:"ILS",maximumFractionDigits:0});

export function DamageTab() {
  const c=calculations[0]!;
  return <div>
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">DETERMINISTIC ENGINE</span><h2>תחשיב נזק</h2></div><span className={`status-badge ${c.status==="locked"?"ok":""}`}>{c.status}</span></div>
      <div className="damage-summary">
        <div><span>ברוטו</span><strong>{money(c.grossCents)}</strong></div>
        <div><span>ניכויים</span><strong>{money(c.deductionsCents)}</strong></div>
        <div className="net"><span>נטו</span><strong>{money(c.netCents)}</strong></div>
      </div>
      <div className="legal-note"><strong>כלל</strong><span>כסף נשמר באגורות. AI רשאי להציע inputs, אך אינו משנה נוסחה או Ruleset.</span></div>
    </section>
    <section className="workspace-card"><h2>מקורות קלט</h2>
      <div className="table"><div className="tr th"><span>ראש נזק</span><span>ערך</span><span>מקור</span><span>מצב</span></div>
        <div className="tr"><span>הפסדי שכר עבר</span><span>₪32,400</span><span>תלושים מאומתים</span><span>אושר</span></div>
        <div className="tr"><span>עזרת צד ג׳</span><span>₪8,000</span><span>קלט עו״ד</span><span>ידני</span></div>
      </div>
    </section>
  </div>;
}
