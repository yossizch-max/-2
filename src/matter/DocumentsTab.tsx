import { documents } from "../lib/demo";
import { StatusBadge } from "../components/StatusBadge";

export function DocumentsTab() {
  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">SOURCE GRAPH</span><h2>מסמכים</h2></div><button className="btn primary">סרוק ועדכן</button></div>
    <div className="table">
      <div className="tr th"><span>קובץ</span><span>קטגוריה</span><span>מצב מקור</span><span>טקסט</span></div>
      {documents.map(d=><button className="tr" key={d.id}>
        <span><b>{d.fileName}</b><small>{d.modifiedAt}</small></span>
        <span>{d.category}</span>
        <span><StatusBadge tone={d.sourceState==="local"?"ok":"warn"}>{d.sourceState}</StatusBadge></span>
        <span>{d.extractionState}</span>
      </button>)}
    </div>
  </section>;
}
