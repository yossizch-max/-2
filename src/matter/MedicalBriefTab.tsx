import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { MedicalBrief, MedicalBriefItem } from "../types";

function ItemRow({item, text}:{item:MedicalBriefItem; text:string}) {
  return <div className="fact-row" key={item.id}>
    <p>{text}</p>
    <small className="quiet">
      {item.pending ? "טרם אושר - הצעת AI" : "אושר ע״י עורך הדין"} · מבוסס על {item.structured.sourceIds?.length ?? 0} מקור/ות
    </small>
    {item.pending && <StatusBadge tone="warn">ממתין לאישור</StatusBadge>}
  </div>;
}

export function MedicalBriefTab({matterId}:{matterId:string}) {
  const {data:brief,loading,error} = useCommand(
    () => commands.get_medical_brief({matterId}) as Promise<MedicalBrief>, [matterId]
  );

  if (loading) return <section className="workspace-card"><p className="quiet">בונה תדריך רפואי...</p></section>;
  if (error || !brief) return <section className="workspace-card"><p className="quiet">שגיאה: {error}</p></section>;

  return <section className="workspace-card">
    <span className="eyebrow">GENERATED · REVIEW BEFORE RELYING</span>
    <h2>תדריך רפואי</h2>
    <p className="quiet">
      נבנה ממידע מאומת בפנקס הרפואי ומהצעות AI שאושרו. פריטים המסומנים "ממתין לאישור" הם הצעת AI שטרם נבדקה -
      אין להסתמך עליהם כעובדה רפואית. המערכת אינה מאבחנת, אינה קובעת קשר סיבתי ואינה מחשבת נכות.
    </p>

    <div style={{marginTop:16}}><h3>היסטוריית טיפול עיקרית</h3>
      {brief.mainTreatmentHistory.length===0 && <p className="quiet">אין עדיין ביקורים שזוהו.</p>}
      {brief.mainTreatmentHistory.map(e=><ItemRow key={e.id} item={e} text={`${e.structured.encounterType ?? "?"} · ${e.structured.provider ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>תלונות מרכזיות</h3>
      {brief.keyComplaints.length===0 && <p className="quiet">אין עדיין תלונות שזוהו.</p>}
      {brief.keyComplaints.map(c=><ItemRow key={c.id} item={c} text={c.structured.complaint ?? ""}/>)}
    </div>

    <div style={{marginTop:16}}><h3>ממצאים אובייקטיביים</h3>
      {brief.objectiveFindings.length===0 && <p className="quiet">אין עדיין ממצאים שזוהו.</p>}
      {brief.objectiveFindings.map(f=><ItemRow key={f.id} item={f} text={f.structured.finding ?? ""}/>)}
    </div>

    <div style={{marginTop:16}}><h3>אבחנות שנרשמו במסמכים</h3>
      {brief.diagnoses.length===0 && <p className="quiet">אין עדיין אבחנות שזוהו.</p>}
      {brief.diagnoses.map(d=><ItemRow key={d.id} item={d} text={`${d.structured.diagnosisText ?? ""} (${d.structured.certainty ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>בדיקות והדמיה</h3>
      {brief.testsImaging.length===0 && <p className="quiet">אין עדיין בדיקות שזוהו.</p>}
      {brief.testsImaging.map(t=><ItemRow key={t.id} item={t} text={`${t.structured.testType ?? "?"} (${t.structured.stage ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>טיפולים</h3>
      {brief.treatments.length===0 && <p className="quiet">אין עדיין טיפולים שזוהו.</p>}
      {brief.treatments.map(t=><ItemRow key={t.id} item={t} text={t.structured.treatmentType ?? ""}/>)}
    </div>

    <div style={{marginTop:16}}><h3>מגבלות תפקודיות/כושר עבודה</h3>
      {brief.functionalWorkLimitations.length===0 && <p className="quiet">אין עדיין מגבלות שזוהו.</p>}
      {brief.functionalWorkLimitations.map(f=><ItemRow key={f.id} item={f} text={`${f.structured.limitation ?? ""} (${f.structured.workCapacityStatus ?? "?"})`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>קביעות נכות</h3>
      <p className="quiet">מוצגות רק קביעות שנרשמו במפורש ע״י גורם מוסמך - TAHRIR אינה מחשבת אחוזי נכות.</p>
      {brief.disabilityDeterminations.length===0 && <p className="quiet">אין עדיין קביעות נכות שזוהו.</p>}
      {brief.disabilityDeterminations.map(d=><ItemRow key={d.id} item={d} text={`${d.structured.determiningBody ?? "?"}: ${typeof d.structured.percentage==="number"?d.structured.percentage+"%":"?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>היסטוריה רפואית קודמת</h3>
      {brief.priorDocumentedHistory.length===0 && <p className="quiet">אין עדיין היסטוריה קודמת שזוהתה.</p>}
      {brief.priorDocumentedHistory.map(p=><ItemRow key={p.id} item={p} text={p.structured.description ?? ""}/>)}
    </div>

    <div style={{marginTop:16}}><h3>חוות דעת רפואיות</h3>
      {brief.medicalOpinions.length===0 && <p className="quiet">אין עדיין חוות דעת שזוהו.</p>}
      {brief.medicalOpinions.map(o=><ItemRow key={o.id} item={o} text={`${o.structured.author ?? "?"}: ${o.structured.opinionText ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>פערי תיעוד אפשריים</h3>
      <p className="quiet">איתות טכני בלבד - אינו מסמן החלמה, נטישת טיפול או היעדר פגיעה.</p>
      {brief.candidateGaps.length===0 && <p className="quiet">לא זוהו פערים.</p>}
      {brief.candidateGaps.map(g=><ItemRow key={g.id} item={g} text={`${g.structured.startDate ?? "?"} - ${g.structured.endDate ?? "?"}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>חומר שלא נמצא בחומר שנקלט</h3>
      {brief.missingEvidenceSignals.length===0 && <p className="quiet">לא זוהה חומר חסר.</p>}
      {brief.missingEvidenceSignals.map(m=><ItemRow key={m.id} item={m} text={`${m.structured.missingType ?? "?"}: ${m.structured.description ?? ""}`}/>)}
    </div>

    <div style={{marginTop:16}}><h3>סתירות הדורשות בדיקה</h3>
      {brief.contradictions.length===0 && <p className="quiet">לא זוהו סתירות.</p>}
      {brief.contradictions.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.itemA ?? ""} ⟷ ${c.structured.itemB ?? ""}`}/>)}
    </div>

    <div className="meta-chips" style={{marginTop:16}}>
      <span>{brief.pendingMedicalReviewCount} פריטי AI ממתינים לבדיקה</span>
    </div>
  </section>;
}
