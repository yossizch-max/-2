import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { MedicalTimelineItem, PriorVsPostIncidentView } from "../types";

const KIND_LABELS: Record<string,string> = {
  medical_encounter: "ביקור (AI - נבדק)",
  medical_complaint: "תלונה (AI - נבדק)",
  medical_finding: "ממצא (AI - נבדק)",
  medical_diagnosis: "אבחנה (AI - נבדק)",
  medical_test: "בדיקה/הדמיה (AI - נבדק)",
  medical_treatment: "טיפול (AI - נבדק)",
  medical_medication: "תרופה (AI - נבדק)",
  medical_referral: "הפניה (AI - נבדק)",
  medical_functional_status: "תפקוד (AI - נבדק)",
  medical_disability_determination: "קביעת נכות (AI - נבדק)",
  medical_prior_history: "היסטוריה קודמת (AI - נבדק)",
  medical_opinion: "חוות דעת (AI - נבדק)",
  medical_ledger_event: "רשומת פנקס רפואי מאומתת",
};

function formatDate(value?: string | null) {
  if (!value) return "תאריך לא ידוע";
  return value.length >= 10 ? value.slice(0,10) : value;
}

function ItemCard({item}:{item:MedicalTimelineItem}) {
  return <div className="proposal" key={`${item.kind}-${item.id}`}>
    <div className="header-actions">
      <strong>{formatDate(item.businessDate)}{item.datePrecision && item.datePrecision!=="exact" ? ` (${item.datePrecision})` : ""} · {item.title}</strong>
      <StatusBadge tone={item.verified?"ok":"warn"}>{item.verified?"מאומת":"נבדק ע״י AI"}</StatusBadge>
    </div>
    <small className="quiet">{KIND_LABELS[item.kind] ?? item.kind}</small>
    {item.description && <p>{item.description}</p>}
  </div>;
}

export function MedicalTimelineTab({matterId}:{matterId:string}) {
  const {data:items,loading,error} = useCommand(
    () => commands.get_medical_timeline({matterId}) as Promise<MedicalTimelineItem[]>, [matterId]
  );
  const [showPriorVsPost,setShowPriorVsPost] = useState(false);
  const [filter,setFilter] = useState("");
  const {data:view,loading:viewLoading} = useCommand(
    () => commands.get_prior_vs_post_incident({matterId, filter: filter.trim() || undefined}) as Promise<PriorVsPostIncidentView>,
    [matterId, showPriorVsPost, filter]
  );

  const dated = items?.filter(i => i.businessDate) ?? [];
  const undated = items?.filter(i => !i.businessDate) ?? [];

  return <section className="workspace-card">
    <div className="header-actions">
      <div><span className="eyebrow">READ-ONLY VIEW</span><h2>ציר זמן רפואי</h2></div>
      <button className="btn secondary" onClick={()=>setShowPriorVsPost(v=>!v)}>
        {showPriorVsPost ? "חזרה לציר הזמן" : "השוואה: לפני/אחרי האירוע"}
      </button>
    </div>
    <p className="quiet">
      תצוגה בלבד - מבוססת על רשומות מאומתות בפנקס הרפואי ועל פריטי AI שאושרו במסך "ראיות רפואיות".
      ממוין לפי תאריך רפואי/עסקי, לא לפי מועד ההזנה למערכת.
    </p>

    {!showPriorVsPost && <>
      {loading && <p className="quiet">טוען ציר זמן...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && items?.length===0 && <p className="quiet">אין עדיין רשומות רפואיות בתיק זה.</p>}
      {dated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      {undated.length>0 && <div style={{marginTop:18}}>
        <h3>תאריך לא ידוע <span className="quiet">({undated.length})</span></h3>
        <p className="quiet">פריטים אלו לא הוצמד להם תאריך במקור - הם נשארים גלויים כאן ולא מוצג עבורם תאריך היום.</p>
        {undated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      </div>}
    </>}

    {showPriorVsPost && <div style={{marginTop:12}}>
      <p className="quiet">
        תצוגה נייטרלית בלבד - אינה קובעת קשר סיבתי. הפריטים ממוינים לפי תאריך האירוע הרשום בתיק ({view?.incidentDate ?? "לא הוגדר"}).
      </p>
      <label>סינון (אופציונלי)<input type="text" value={filter} onChange={e=>setFilter(e.target.value)} placeholder="לדוגמה: גב, ברך"/></label>
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
        <h3>לא ניתן למיין (תאריך לא ידוע או שאין תאריך אירוע בתיק) <span className="quiet">({view.undated.length})</span></h3>
        {view.undated.map(item => <ItemCard key={`${item.kind}-${item.id}`} item={item}/>)}
      </div>}
    </div>}
  </section>;
}
