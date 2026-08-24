import { loadActionItems } from "../lib/actionCenter";
import { useCommand } from "../lib/hooks";

const groups=[["critical","קריטי"],["review","דורש אישור"],["waiting","ממתין לאחרים"],["resume","להמשך"],["new","חדש"]] as const;

export function TodayPage() {
  const {data:today,loading,error}=useCommand(loadActionItems,[]);
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">יום עבודה</span><h1>היום</h1><p>מה יכול לפגוע היום, מה דורש אישור ומה ממתין.</p></div></div>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && today?.length===0 && <p className="quiet">אין פריטים פתוחים כרגע.</p>}
    {groups.map(([kind,label])=>{
      const rows=today?.filter(x=>x.kind===kind) ?? [];
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
