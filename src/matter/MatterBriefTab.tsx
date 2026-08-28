import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { MatterBrief, MatterBriefItem } from "../types";

function ItemRow({item, text}:{item:MatterBriefItem; text:string}) {
  return <div className="fact-row" key={item.id}>
    <p>{text}</p>
    <small className="quiet">
      {item.pending ? "טרם אושר - נשמר לצורך בדיקה" : "אושר ע״י עורך הדין"} ·
      {" "}מבוסס על {item.structured.sourceIds?.length ?? 0} מקור/ות
    </small>
    {item.pending && <StatusBadge tone="warn">ממתין לאישור</StatusBadge>}
  </div>;
}

export function MatterBriefTab({matterId}:{matterId:string}) {
  const {data:brief,loading,error} = useCommand(
    () => commands.get_matter_brief({matterId}) as Promise<MatterBrief>, [matterId]
  );

  if (loading) return <section className="workspace-card"><p className="quiet">בונה תדריך תיק...</p></section>;
  if (error || !brief) return <section className="workspace-card"><p className="quiet">שגיאה: {error}</p></section>;

  return <section className="workspace-card">
    <span className="eyebrow">GENERATED · REVIEW BEFORE RELYING</span>
    <h2>תדריך תיק</h2>
    <p className="quiet">
      נבנה ממידע מאומת/מוכר ומהצעות AI שאושרו. פריטים המסומנים "ממתין לאישור" הם תוכן AI שטרם נבדק -
      אין להסתמך עליהם כעובדה. לבדיקת המקור המלא יש לפתוח את לשונית "הבנת התיק" או "מסמכים".
    </p>

    <div style={{marginTop:16}}>
      <h3>צדדים וישויות</h3>
      {brief.parties.length===0 && brief.entities.length===0 && <p className="quiet">אין עדיין צדדים או ישויות מזוהות.</p>}
      {brief.parties.map(p=><div className="fact-row" key={p.id}><strong>{p.displayName}</strong><small className="quiet"> · {p.role}</small></div>)}
      {brief.entities.map(e=><ItemRow key={e.id} item={e} text={`${e.structured.displayName ?? "ישות"} (${e.structured.entityType ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}>
      <h3>מה קרה - כרונולוגיה מרכזית</h3>
      {brief.chronology.length===0 && <p className="quiet">אין עדיין אירועים בציר הזמן.</p>}
      {brief.chronology.map(item=><div className="fact-row" key={`${item.kind}-${item.id}`}>
        <strong>{item.businessDate.slice(0,10)} · {item.title}</strong>
        <StatusBadge tone={item.verified?"ok":"warn"}>{item.verified?"מאומת":"נבדק ע״י AI"}</StatusBadge>
        {item.description && <p>{item.description}</p>}
      </div>)}
    </div>

    <div style={{marginTop:16}}>
      <h3>טענות ופלוגתאות נוכחיות</h3>
      {brief.claims.length===0 && <p className="quiet">אין עדיין טענות שזוהו.</p>}
      {brief.claims.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.assertedBy ?? "?"}: ${c.structured.statement ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}>
      <h3>סכומים חשובים</h3>
      {brief.amounts.length===0 && <p className="quiet">אין עדיין סכומים שזוהו.</p>}
      {brief.amounts.map(a=><ItemRow key={a.id} item={a}
        text={`${a.structured.amountType ?? "?"}: ${typeof a.structured.amountCents==="number"?(a.structured.amountCents/100).toLocaleString("he-IL",{style:"currency",currency:a.structured.currency||"ILS"}):"?"}`}/>)}
    </div>

    <div style={{marginTop:16}}>
      <h3>סתירות מרכזיות</h3>
      <p className="quiet">רשימת סתירות לבדיקה בלבד; המערכת אינה מכריעה איזה מקור נכון.</p>
      {brief.contradictions.length===0 && <p className="quiet">אין עדיין סתירות שזוהו.</p>}
      {brief.contradictions.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.itemA ?? ""} ⟷ ${c.structured.itemB ?? ""} — ${c.structured.reason ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}>
      <h3>מידע חסר / שאלות מוצעות</h3>
      {brief.missingInformation.length===0 && <p className="quiet">אין כרגע שאלות מוצעות.</p>}
      {brief.missingInformation.map(q=><ItemRow key={q.id} item={q} text={q.structured.question ?? ""}/>)}
    </div>

    <div className="meta-chips" style={{marginTop:16}}>
      <span>{brief.verifiedFactCount} עובדות מאומתות</span>
      <span>{brief.openConflictCount} סתירות פתוחות בין עובדות</span>
    </div>
  </section>;
}
