import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { WageBrief, WageBriefItem } from "../types";

function ItemRow({item, text}:{item:WageBriefItem; text:string}) {
  return <div className="fact-row" key={item.id}>
    <p>{text}</p>
    <small className="quiet">
      {item.pending ? "טרם אושר - הצעת AI" : "אושר ע״י עורך הדין"} · מבוסס על {item.structured.sourceIds?.length ?? 0} מקור/ות
    </small>
    {item.pending && <StatusBadge tone="warn">ממתין לאישור</StatusBadge>}
  </div>;
}

export function WageBriefTab({matterId}:{matterId:string}) {
  const {data:brief,loading,error} = useCommand(
    () => commands.get_wage_brief({matterId}) as Promise<WageBrief>, [matterId]
  );

  if (loading) return <section className="workspace-card"><p className="quiet">בונה תדריך שכר...</p></section>;
  if (error || !brief) return <section className="workspace-card"><p className="quiet">שגיאה: {error}</p></section>;

  return <section className="workspace-card">
    <span className="eyebrow">GENERATED · REVIEW BEFORE RELYING</span>
    <h2>תדריך שכר וכלכלה</h2>
    <p className="quiet">
      נבנה ממידע מאומת בפנקס השכר ומהצעות AI שאושרו. פריטים המסומנים "ממתין לאישור" הם הצעת AI שטרם נבדקה -
      אין להסתמך עליהם כעובדה. המערכת אינה מחשבת אובדן שכר בפועל, אובדן כושר השתכרות, היוון, או אובדן פנסיה.
    </p>

    <div style={{marginTop:16}}><h3>העסקה</h3>
      {brief.employment.length===0 && <p className="quiet">אין עדיין רשומות העסקה שזוהו.</p>}
      {brief.employment.map(e=><ItemRow key={e.id} item={e} text={`${e.structured.employer ?? "?"} · ${e.structured.role ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>הכנסה</h3>
      {brief.income.length===0 && <p className="quiet">אין עדיין רשומות הכנסה שזוהו.</p>}
      {brief.income.map(i=><ItemRow key={i.id} item={i} text={`${typeof i.structured.amountCents==="number"?(i.structured.amountCents/100).toFixed(2)+" ש\"ח":"?"} (${i.structured.amountBasis ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>תלושי שכר</h3>
      {brief.payslips.length===0 && <p className="quiet">אין עדיין תלושים שזוהו.</p>}
      {brief.payslips.map(p=><ItemRow key={p.id} item={p} text={`${p.structured.month ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>הכנסה שנתית של שכיר</h3>
      {brief.annualIncome.length===0 && <p className="quiet">אין עדיין הכנסה שנתית שזוהתה.</p>}
      {brief.annualIncome.map(a=><ItemRow key={a.id} item={a} text={`${a.structured.sourceType ?? "?"} · ${a.structured.year ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>אישורי מעסיק</h3>
      {brief.employerConfirmations.length===0 && <p className="quiet">אין עדיין אישורי מעסיק שזוהו.</p>}
      {brief.employerConfirmations.map(e=><ItemRow key={e.id} item={e} text={`${e.structured.employer ?? "?"}: ${e.structured.statedSalaryText ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>הכנסה כעצמאי</h3>
      <p className="quiet">הכנסות, הוצאות ורווח מוצגים כמושגים נפרדים - אינם מוזגים זה בזה.</p>
      {brief.selfEmployedIncome.length===0 && <p className="quiet">אין עדיין הכנסת עצמאי שזוהתה.</p>}
      {brief.selfEmployedIncome.map(s=><ItemRow key={s.id} item={s} text={`${s.structured.documentType ?? "?"} · ${s.structured.taxYear ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>הפרשות פנסיוניות/סוציאליות</h3>
      {brief.pensionContributions.length===0 && <p className="quiet">אין עדיין הפרשות שזוהו.</p>}
      {brief.pensionContributions.map(p=><ItemRow key={p.id} item={p} text={p.structured.pensionComponent ?? "הפרשה פנסיונית"}/>)}
    </div>

    <div style={{marginTop:16}}><h3>היעדרויות</h3>
      {brief.absences.length===0 && <p className="quiet">אין עדיין היעדרויות שזוהו.</p>}
      {brief.absences.map(a=><ItemRow key={a.id} item={a} text={`${a.structured.startDate ?? "?"} - ${a.structured.endDate ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>תעודות מחלה</h3>
      {brief.sickLeave.length===0 && <p className="quiet">אין עדיין תעודות מחלה שזוהו.</p>}
      {brief.sickLeave.map(s=><ItemRow key={s.id} item={s} text={`${s.structured.startDate ?? "?"} - ${s.structured.endDate ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>מגבלות עבודה</h3>
      {brief.workLimitations.length===0 && <p className="quiet">אין עדיין מגבלות שזוהו.</p>}
      {brief.workLimitations.map(w=><ItemRow key={w.id} item={w} text={`${w.structured.limitation ?? ""} (${w.structured.workCapacityStatus ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>שינויים תעסוקתיים</h3>
      <p className="quiet">מוצג רק כפי שתועד במקור - TAHRIR אינה קובעת קשר סיבתי לאירוע.</p>
      {brief.employmentChanges.length===0 && <p className="quiet">אין עדיין שינויים שזוהו.</p>}
      {brief.employmentChanges.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.changeType ?? "?"}: ${c.structured.description ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>תשלומים/גמלאות</h3>
      {brief.benefitPayments.length===0 && <p className="quiet">אין עדיין תשלומים שזוהו.</p>}
      {brief.benefitPayments.map(b=><ItemRow key={b.id} item={b} text={`${b.structured.paymentType ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>חומר שלא נמצא בחומר שנקלט</h3>
      {brief.missingEvidenceSignals.length===0 && <p className="quiet">לא זוהה חומר חסר.</p>}
      {brief.missingEvidenceSignals.map(m=><ItemRow key={m.id} item={m} text={`${m.structured.gapType ?? "?"}: ${m.structured.description ?? ""}`}/>)}
    </div>

    <div className="meta-chips" style={{marginTop:16}}>
      <span>{brief.pendingWageReviewCount} פריטי AI ממתינים לבדיקה</span>
    </div>
  </section>;
}
