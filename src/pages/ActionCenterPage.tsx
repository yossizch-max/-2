import { today } from "../lib/demo";
export function ActionCenterPage() {
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">OPERATIONS</span><h1>מרכז פעולה</h1><p>ה־backlog התפעולי המלא, עם פעולה ראשית אחת לכל פריט.</p></div></div>
    <div className="filterbar"><button className="active">הכל</button><button>משפטי</button><button>Review</button><button>ממתין</button><button>Stale</button></div>
    <div className="dense-list">{today.map(x=><button className="work-row" key={x.id}><div><strong>{x.title}</strong><small>{x.matterTitle} · {x.subtitle}</small></div><span>{x.actionLabel}</span></button>)}</div>
  </div>;
}
