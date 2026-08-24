import { legalDocuments } from "../lib/demo";
export function LegalDocumentsTab() {
  return <div>
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">LEGAL DOCUMENTS</span><h2>מסמכים משפטיים</h2></div><button className="btn primary">טיוטה חדשה</button></div>
      {legalDocuments.map(d=><div className="legal-card" key={d.id}><div><strong>{d.title}</strong><small>{d.kind} · {d.status}</small></div><button className="btn secondary">פתח עורך</button></div>)}
    </section>
    <section className="legal-paper"><h2>מכתב דרישה · תצוגת עורך</h2><p>כל פסקה עובדתית מקבלת provenance. שינוי ידני יוצר child version. אישור חוסם שינוי שקט.</p><div className="paper-paragraph"><p>מרשנו נפגע בתאונת דרכים ביום 14.04.2026...</p><button className="source-link">מקורות: f2 · מסמך v2</button></div></section>
  </div>;
}
