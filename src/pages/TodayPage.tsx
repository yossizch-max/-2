import { today } from "../lib/demo";
const groups=[["critical","קריטי"],["review","דורש אישור"],["waiting","ממתין לאחרים"],["resume","להמשך"],["new","חדש"]] as const;
export function TodayPage() {
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">יום עבודה</span><h1>היום</h1><p>מה יכול לפגוע היום, מה דורש אישור ומה ממתין.</p></div></div>
    {groups.map(([kind,label])=>{
      const rows=today.filter(x=>x.kind===kind);
      if(!rows.length) return null;
      return <section className="work-section" key={kind}>
        <div className="section-head"><h2>{label}</h2><span>{rows.length}</span></div>
        <div className="dense-list">{rows.map(x=><button className="work-row" key={x.id}>
          <div><strong>{x.title}</strong><small>{x.matterTitle} · {x.subtitle}</small></div><span>{x.actionLabel}</span>
        </button>)}</div>
      </section>;
    })}
  </div>;
}
