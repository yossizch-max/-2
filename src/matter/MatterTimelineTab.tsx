import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { TimelineItem } from "../types";

const KIND_LABELS: Record<string,string> = {
  understanding_event: "אירוע (AI - נבדק, לא מאומת)",
  medical_event: "אירוע רפואי מאומת",
  wage_record: "רשומת שכר מאומתת",
  insurance_status: "סטטוס תביעת ביטוח",
  negotiation_event: "אירוע מו״מ",
  calendar_event: "יומן",
};

function formatDate(value: string) {
  return value.length >= 10 ? value.slice(0,10) : value;
}

export function MatterTimelineTab({matterId}:{matterId:string}) {
  const {data:items,loading,error} = useCommand(
    () => commands.get_matter_timeline({matterId}) as Promise<TimelineItem[]>, [matterId]
  );

  return <section className="workspace-card">
    <span className="eyebrow">READ-ONLY VIEW</span>
    <h2>ציר זמן מאוחד</h2>
    <p className="quiet">
      תצוגה בלבד, ללא כתיבה - מבוססת על רשומות מאומתות/מוכרות בתיק ועל אירועי AI שאושרו במסך "הבנת התיק".
      הרשומות ממוינות לפי תאריך עסקי/אירוע, לא לפי מועד ההזנה למערכת.
    </p>
    {loading && <p className="quiet">טוען ציר זמן...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && items?.length===0 && <p className="quiet">אין עדיין רשומות לציר הזמן של תיק זה.</p>}
    <div className="timeline">
      {items?.map(item => <div className="proposal" key={`${item.kind}-${item.id}`}>
        <div className="header-actions">
          <strong>
            {formatDate(item.businessDate)}
            {item.datePrecision && item.datePrecision!=="exact" ? ` (${item.datePrecision})` : ""}
            {" "}· {item.title}
          </strong>
          <StatusBadge tone={item.verified?"ok":"warn"}>{item.verified?"מאומת":"נבדק ע״י AI"}</StatusBadge>
        </div>
        <small className="quiet">{KIND_LABELS[item.kind] ?? item.kind}</small>
        {item.description && <p>{item.description}</p>}
      </div>)}
    </div>
  </section>;
}
