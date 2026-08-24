import { matters } from "../lib/demo";
export function MattersPage({onOpen}:{onOpen:(id:string)=>void}) {
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">MATTERS</span><h1>תיקים</h1><p>התיקיות נשארות מקור האמת. TAHRIR מוסיפה ידע, פעולה ומקור.</p></div><button className="btn primary">תיק חדש</button></div>
    <div className="matter-list">{matters.map(m=><button className="matter-row" onClick={()=>onOpen(m.id)} key={m.id}>
      <div className="matter-avatar">{m.title.slice(0,1)}</div>
      <div className="matter-main"><span className="eyebrow">{m.internalNumber}</span><strong>{m.title}</strong><small>{m.documentCount} מסמכים · {m.verifiedFactCount} עובדות · {m.pendingReviewCount} לבדיקה</small></div>
      <span className="chev">←</span>
    </button>)}</div>
  </div>;
}
