import { useState } from "react";
import { loadActionItems } from "../lib/actionCenter";
import { useCommand } from "../lib/hooks";

const FILTERS=[["all","הכל"],["critical","משפטי"],["review","Review"],["waiting","ממתין"]] as const;

export function ActionCenterPage() {
  const {data:today,loading,error}=useCommand(loadActionItems,[]);
  const [filter,setFilter]=useState<typeof FILTERS[number][0]>("all");
  const rows=(today??[]).filter(x=>filter==="all"||x.kind===filter);

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">OPERATIONS</span><h1>מרכז פעולה</h1><p>ה־backlog התפעולי המלא, עם פעולה ראשית אחת לכל פריט.</p></div></div>
    <div className="filterbar">{FILTERS.map(([key,label])=>
      <button key={key} className={filter===key?"active":""} onClick={()=>setFilter(key)}>{label}</button>
    )}</div>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && rows.length===0 && <p className="quiet">אין פריטים.</p>}
    <div className="dense-list">{rows.map(x=><button className="work-row" key={x.id}><div><strong>{x.title}</strong><small>{x.matterTitle} · {x.subtitle}</small></div><span>{x.actionLabel}</span></button>)}</div>
  </div>;
}
