import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { LiabilityBrief, LiabilityBriefItem, LiabilityMatrix, LiabilityRegime } from "../types";

const REGIME_LABELS: Record<LiabilityRegime, string> = {
  ftl_road_accident: "תאונת דרכים (חוק פלת״ד - אחריות ברובה ללא תלות ברשלנות)",
  ordinary_negligence: "רשלנות כללית",
  unknown_requires_review: "יש להגדיר/לאשר את מסלול האחריות המשפטי",
};

function ItemRow({item, text}:{item:LiabilityBriefItem; text:string}) {
  return <div className="fact-row" key={item.id}>
    <p>{text}</p>
    <small className="quiet">
      {item.pending ? "טרם אושר - הצעת AI" : "אושר ע״י עורך הדין"} · מבוסס על {item.structured.sourceIds?.length ?? 0} מקור/ות
    </small>
    {item.pending && <StatusBadge tone="warn">ממתין לאישור</StatusBadge>}
  </div>;
}

function MatrixView({matterId}:{matterId:string}) {
  const {data:matrix,loading,error} = useCommand(
    () => commands.get_liability_matrix({matterId}) as Promise<LiabilityMatrix>, [matterId]
  );
  if (loading) return <p className="quiet">בונה מטריצת אחריות...</p>;
  if (error || !matrix) return <p className="quiet">שגיאה: {error}</p>;
  if (matrix.rows.length===0) return <p className="quiet">אין עדיין מספיק גרסאות/ראיות מאושרות לבניית מטריצה.</p>;

  return <>
    <p className="quiet">
      מטריצה נייטרלית סביב סוגיות עובדתיות מרכזיות. TAHRIR אינה בוחרת גרסה נכונה ואינה מפיקה ציון אשם - "סתירה לא פתורה" הוא איתות טקסטואלי בלבד.
    </p>
    {matrix.rows.map((row,i) => <div className="proposal" key={i}>
      <div className="header-actions">
        <strong>{row.issue ?? "ללא סוגיה משויכת"}</strong>
        {row.unresolvedConflict && <StatusBadge tone="warn">סתירה לא פתורה</StatusBadge>}
      </div>
      {row.versions.length>0 && <div style={{marginTop:8}}><small className="quiet">גרסאות צדדים:</small>
        {row.versions.map(v=><p key={v.id}>{v.structured.assertedBy}: {v.structured.statement}</p>)}
      </div>}
      {row.witnesses.length>0 && <div style={{marginTop:8}}><small className="quiet">עדויות:</small>
        {row.witnesses.map(w=><p key={w.id}>{w.structured.witness}: {w.structured.statement}</p>)}
      </div>}
      {row.objectiveEvidence.length>0 && <div style={{marginTop:8}}><small className="quiet">ראיות אובייקטיביות:</small>
        {row.objectiveEvidence.map(e=><p key={e.id}>{e.structured.evidenceType}: {e.structured.description}</p>)}
      </div>}
    </div>)}
  </>;
}

export function LiabilityBriefTab({matterId}:{matterId:string}) {
  const [showMatrix,setShowMatrix] = useState(false);
  const {data:brief,loading,error} = useCommand(
    () => commands.get_liability_brief({matterId}) as Promise<LiabilityBrief>, [matterId]
  );

  return <section className="workspace-card">
    <div className="header-actions">
      <div><span className="eyebrow">GENERATED · REVIEW BEFORE RELYING</span><h2>תדריך אחריות</h2></div>
      <button className="btn secondary" onClick={()=>setShowMatrix(v=>!v)}>
        {showMatrix ? "חזרה לתדריך" : "מטריצת ראיות אחריות"}
      </button>
    </div>
    {brief && <p className="quiet"><strong>מסלול אחריות: </strong>{REGIME_LABELS[brief.regime]}</p>}
    <p className="quiet">
      נבנה ממידע מאומת בפנקס האחריות ומהצעות AI שאושרו. פריטים המסומנים "ממתין לאישור" הם הצעת AI שטרם נבדקה -
      אין להסתמך עליהם כעובדה. המערכת אינה קובעת אשם, רשלנות, אחוז רשלנות תורמת, קשר סיבתי משפטי, או אמינות עדים.
    </p>

    {showMatrix && <MatrixView matterId={matterId}/>}

    {!showMatrix && <>
      {loading && <p className="quiet">טוען תדריך...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {brief && <>
        <div style={{marginTop:16}}><h3>גרסאות הצדדים</h3>
          {brief.partyVersions.length===0 && <p className="quiet">אין עדיין גרסאות שזוהו.</p>}
          {brief.partyVersions.map(v=><ItemRow key={v.id} item={v} text={`${v.structured.assertedBy ?? "?"}: ${v.structured.statement ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>עדויות</h3>
          {brief.witnesses.length===0 && <p className="quiet">אין עדיין עדויות שזוהו.</p>}
          {brief.witnesses.map(w=><ItemRow key={w.id} item={w} text={`${w.structured.witness ?? "?"}: ${w.structured.statement ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>ראיות זירה אובייקטיביות</h3>
          {brief.sceneEvidence.length===0 && <p className="quiet">אין עדיין ראיות זירה שזוהו.</p>}
          {brief.sceneEvidence.map(s=><ItemRow key={s.id} item={s} text={`${s.structured.evidenceType ?? "?"}: ${s.structured.description ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>חומר משטרתי</h3>
          {brief.policeEvidence.length===0 && <p className="quiet">אין עדיין חומר משטרתי שזוהה.</p>}
          {brief.policeEvidence.map(p=><ItemRow key={p.id} item={p} text={`${p.structured.reportType ?? "?"}: ${p.structured.factualContent ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>נזק לרכב</h3>
          {brief.vehicleDamage.length===0 && <p className="quiet">אין עדיין נזק לרכב שזוהה.</p>}
          {brief.vehicleDamage.map(v=><ItemRow key={v.id} item={v} text={`${v.structured.vehicle ?? "?"}: ${v.structured.documentedCondition ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>תמונות/סרטונים</h3>
          {brief.photoVideoEvidence.length===0 && <p className="quiet">אין עדיין תמונות/סרטונים שזוהו.</p>}
          {brief.photoVideoEvidence.map(m=><ItemRow key={m.id} item={m} text={m.structured.description ?? ""}/>)}
        </div>

        <div style={{marginTop:16}}><h3>חוות דעת מומחה</h3>
          {brief.expertOpinions.length===0 && <p className="quiet">אין עדיין חוות דעת שזוהו.</p>}
          {brief.expertOpinions.map(o=><ItemRow key={o.id} item={o} text={`${o.structured.expert ?? "?"}: ${o.structured.opinionText ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>הודאות</h3>
          {brief.admissions.length===0 && <p className="quiet">אין עדיין הודאות שזוהו.</p>}
          {brief.admissions.map(a=><ItemRow key={a.id} item={a} text={`${a.structured.assertedBy ?? "?"}: ${a.structured.statement ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>עמדת המבטח</h3>
          {brief.insurerPositions.length===0 && <p className="quiet">אין עדיין עמדת מבטח שזוהתה.</p>}
          {brief.insurerPositions.map(i=><ItemRow key={i.id} item={i} text={`${i.structured.position ?? "?"}: ${i.structured.detail ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>קביעות בית משפט</h3>
          <p className="quiet">מוצגת רמת הקביעה בדיוק כפי שתועדה - הערת ביניים לעולם אינה מוצגת כפסק דין סופי.</p>
          {brief.courtFindings.length===0 && <p className="quiet">אין עדיין קביעות שזוהו.</p>}
          {brief.courtFindings.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.findingType ?? "?"}: ${c.structured.description ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>סוגיות אחריות פתוחות</h3>
          <p className="quiet">סוגיה עובדתית/כיסוי ניטרלית בלבד - אינה מסמנת מי צודק.</p>
          {brief.liabilityIssues.length===0 && <p className="quiet">לא זוהו סוגיות פתוחות.</p>}
          {brief.liabilityIssues.map(i=><ItemRow key={i.id} item={i} text={`${i.structured.issueType ?? "?"}: ${i.structured.description ?? ""}`}/>)}
        </div>

        <div style={{marginTop:16}}><h3>סתירות הדורשות בדיקה</h3>
          {brief.contradictions.length===0 && <p className="quiet">לא זוהו סתירות.</p>}
          {brief.contradictions.map(c=><ItemRow key={c.id} item={c} text={`${c.structured.itemA ?? ""} ⟷ ${c.structured.itemB ?? ""}`}/>)}
        </div>

        <div className="meta-chips" style={{marginTop:16}}>
          <span>{brief.pendingLiabilityReviewCount} פריטי AI ממתינים לבדיקה</span>
        </div>
      </>}
    </>}
  </section>;
}
