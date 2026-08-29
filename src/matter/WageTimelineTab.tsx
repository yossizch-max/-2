import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { WageTimelineItem, WageComparisonView } from "../types";

const KIND_LABELS: Record<string,string> = {
  wage_employment: "העסקה (AI - נבדק)",
  wage_income: "הכנסה (AI - נבדק)",
  wage_payslip: "תלוש שכר (AI - נבדק)",
  wage_annual_income: "הכנסה שנתית (AI - נבדק)",
  wage_absence: "היעדרות (AI - נבדק)",
  wage_sick_leave: "תעודת מחלה (AI - נבדק)",
  wage_work_limitation: "מגבלת עבודה (AI - נבדק)",
  wage_employment_change: "שינוי תעסוקתי (AI - נבדק)",
  wage_benefit_payment: "תשלום/גמלה (AI - נבדק)",
  wage_ledger_record: "רשומת פנקס שכר מאומתת",
};

function formatDate(value?: string | null) {
  if (!value) return "תאריך לא ידוע";
  return value.length >= 10 ? value.slice(0,10) : value;
}

function ItemCard({item}:{item:WageTimelineItem}) {
  return <div className="proposal" key={`${item.kind}-${item.id}`}>
    <div className="header-actions">
      <strong>{formatDate(item.businessDate)} · {item.title}</strong>
      <StatusBadge tone={item.verified?"ok":"warn"}>{item.verified?"מאומת":"נבדק ע״י AI"}</StatusBadge>
    </div>
    <small className="quiet">{KIND_LABELS[item.kind] ?? item.kind}</small>
    {item.description && <p>{item.description}</p>}
  </div>;
}

export function WageTimelineTab({matterId}:{matterId:string}) {
  const {data:items,loading,error} = useCommand(
    () => commands.get_wage_timeline({matterId}) as Promise<WageTimelineItem[]>, [matterId]
  );
  const [showComparison,setShowComparison] = useState(false);
  const [filter,setFilter] = useState("");
  const {data:view,loading:viewLoading} = useCommand(
    () => commands.get_wage_comparison({matterId, filter: filter.trim() || undefined}) as Promise<WageComparisonView>,
    [matterId, showComparison, filter]
  );

  const dated = items?.filter(i => i.businessDate) ?? [];
  const undated = items?.filter(i => !i.businessDate) ?? [];

  return <section className="workspace-card">
    <div className="header-actions">
      <div><span className="eyebrow">READ-ONLY VIEW</span><h2>ציר זמן שכר</h2></div>
      <button className="btn secondary" onClick={()=>setShowComparison(v=>!v)}>
        {showComparison ? "חזרה לציר הזמן" : "השוואה: לפני/אחרי האירוע"}
      </button>
    </div>
    <p className="quiet">
      תצוגה בלבד - מבוססת על רשומות מאומתות בפנקס השכר ועל פריטי AI שאושרו במסך "ראיות שכר וכלכליות".
      ממוין לפי תקופה עסקית, לא לפי מועד ההזנה למערכת.
    </p>

    {!showComparison && <>
      {loading && <p className="quiet">טוען ציר זמן...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && items?.length===0 && <p className="quiet">אין עדיין רשומות שכר בתיק זה.</p>}
      {dated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      {undated.length>0 && <div style={{marginTop:18}}>
        <h3>תאריך לא ידוע <span className="quiet">({undated.length})</span></h3>
        <p className="quiet">פריטים אלו לא הוצמדה להם תקופה במקור - הם נשארים גלויים כאן ולא מוצג עבורם תאריך היום.</p>
        {undated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      </div>}
    </>}

    {showComparison && <div style={{marginTop:12}}>
      <p className="quiet">
        תצוגה נייטרלית בלבד - אינה מחשבת אובדן שכר ואינה קובעת קשר סיבתי. הפריטים ממוינים לפי תאריך האירוע הרשום בתיק ({view?.incidentDate ?? "לא הוגדר"}).
      </p>
      <label>סינון (אופציונלי)<input type="text" value={filter} onChange={e=>setFilter(e.target.value)} placeholder="לדוגמה: מעסיק, תלוש"/></label>
      {viewLoading && <p className="quiet">טוען השוואה...</p>}
      {view && <div className="grid-2">
        <div>
          <h3>תועד לפני האירוע <span className="quiet">({view.documentedBefore.length})</span></h3>
          {view.documentedBefore.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
        </div>
        <div>
          <h3>תועד באירוע/אחריו <span className="quiet">({view.documentedAfter.length})</span></h3>
          {view.documentedAfter.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
        </div>
      </div>}
      {view && view.undated.length>0 && <div style={{marginTop:18}}>
        <h3>לא ניתן למיין (תקופה לא ידועה או שאין תאריך אירוע בתיק) <span className="quiet">({view.undated.length})</span></h3>
        {view.undated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      </div>}
    </div>}
  </section>;
}
