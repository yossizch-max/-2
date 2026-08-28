import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiProposal } from "../types";

const CAPABILITY = "extract_medical_evidence";

const SECTION_ORDER = [
  "medical_encounter", "medical_complaint", "medical_finding", "medical_diagnosis",
  "medical_test", "medical_treatment", "medical_medication", "medical_referral",
  "medical_functional_status", "medical_disability_determination", "medical_prior_history",
  "medical_opinion", "medical_gap_signal", "medical_missing_evidence_signal", "medical_contradiction",
] as const;

const SECTION_LABELS: Record<string, string> = {
  medical_encounter: "ביקורים ואשפוזים",
  medical_complaint: "תלונות",
  medical_finding: "ממצאים",
  medical_diagnosis: "אבחנות",
  medical_test: "בדיקות והדמיה",
  medical_treatment: "טיפולים",
  medical_medication: "תרופות",
  medical_referral: "הפניות",
  medical_functional_status: "תפקוד וכושר עבודה",
  medical_disability_determination: "קביעות נכות",
  medical_prior_history: "היסטוריה רפואית קודמת",
  medical_opinion: "חוות דעת רפואיות",
  medical_gap_signal: "פערי תיעוד אפשריים",
  medical_missing_evidence_signal: "חומר שלא נמצא",
  medical_contradiction: "סתירות אפשריות",
};

function fieldValue(value?: string | number | null) {
  if (value === null || value === undefined || value === "") return "לא ידוע";
  return String(value);
}

function ItemPreview({proposal}:{proposal:AiProposal}) {
  const s = proposal.structured;
  switch (proposal.proposalKind) {
    case "medical_encounter":
      return <dl className="profile-fields">
        <div><dt>סוג ביקור</dt><dd>{fieldValue(s.encounterType)}</dd></div>
        <div><dt>גורם מטפל</dt><dd>{fieldValue(s.provider)}{s.institution?` · ${s.institution}`:""}</dd></div>
        <div><dt>תאריך</dt><dd>{fieldValue(s.eventDate)}{s.datePrecision&&s.datePrecision!=="exact"?` (${s.datePrecision})`:""}</dd></div>
        {s.documentDate && <div><dt>תאריך המסמך</dt><dd>{s.documentDate}</dd></div>}
      </dl>;
    case "medical_complaint":
      return <dl className="profile-fields">
        <div><dt>תלונה שדווחה</dt><dd>{fieldValue(s.complaint)}</dd></div>
        {s.bodyRegion && <div><dt>אזור בגוף</dt><dd>{s.bodyRegion}</dd></div>}
        {s.severity && <div><dt>חומרה</dt><dd>{s.severity}</dd></div>}
      </dl>;
    case "medical_finding":
      return <dl className="profile-fields">
        <div><dt>ממצא שנרשם</dt><dd>{fieldValue(s.finding)}</dd></div>
        {s.bodyRegion && <div><dt>אזור בגוף</dt><dd>{s.bodyRegion}</dd></div>}
      </dl>;
    case "medical_diagnosis":
      return <dl className="profile-fields">
        <div><dt>אבחנה שנרשמה במסמך</dt><dd>{fieldValue(s.diagnosisText)}</dd></div>
        <div><dt>רמת ודאות</dt><dd>{fieldValue(s.certainty)}</dd></div>
        {s.provider && <div><dt>גורם מאבחן</dt><dd>{s.provider}</dd></div>}
      </dl>;
    case "medical_test":
      return <dl className="profile-fields">
        <div><dt>סוג בדיקה</dt><dd>{fieldValue(s.testType)}</dd></div>
        <div><dt>שלב</dt><dd>{fieldValue(s.stage)}</dd></div>
        {s.interpretation && <div><dt>פרשנות</dt><dd>{s.interpretation}</dd></div>}
      </dl>;
    case "medical_treatment":
      return <dl className="profile-fields">
        <div><dt>סוג טיפול</dt><dd>{fieldValue(s.treatmentType)}</dd></div>
        <div><dt>תאריך</dt><dd>{fieldValue(s.date)}</dd></div>
      </dl>;
    case "medical_medication":
      return <dl className="profile-fields">
        <div><dt>תרופה</dt><dd>{fieldValue(s.medication)}</dd></div>
        <div><dt>מינון</dt><dd>{fieldValue(s.dosage)}</dd></div>
        <div><dt>סטטוס</dt><dd>{fieldValue(s.status)}</dd></div>
      </dl>;
    case "medical_referral":
      return <dl className="profile-fields">
        <div><dt>הפניה</dt><dd>{fieldValue(s.planType)}</dd></div>
        {s.target && <div><dt>יעד</dt><dd>{s.target}</dd></div>}
      </dl>;
    case "medical_functional_status":
      return <dl className="profile-fields">
        <div><dt>מגבלה</dt><dd>{fieldValue(s.limitation)}</dd></div>
        <div><dt>סטטוס כושר עבודה</dt><dd>{fieldValue(s.workCapacityStatus)}</dd></div>
      </dl>;
    case "medical_disability_determination":
      return <dl className="profile-fields">
        <div><dt>גורם קובע</dt><dd>{fieldValue(s.determiningBody)}</dd></div>
        <div><dt>אחוז נכות</dt><dd>{typeof s.percentage==="number"?`${s.percentage}%`:"לא ידוע"}</dd></div>
        <div><dt>זמניות</dt><dd>{fieldValue(s.durationType)}</dd></div>
      </dl>;
    case "medical_prior_history":
      return <dl className="profile-fields">
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
        {s.bodyRegion && <div><dt>אזור בגוף</dt><dd>{s.bodyRegion}</dd></div>}
      </dl>;
    case "medical_opinion":
      return <dl className="profile-fields">
        <div><dt>סוג חוות דעת</dt><dd>{fieldValue(s.opinionType)}</dd></div>
        <div><dt>נוסח</dt><dd>{fieldValue(s.opinionText)}</dd></div>
        <div><dt>נכתב ע״י</dt><dd>{fieldValue(s.author)}</dd></div>
      </dl>;
    case "medical_gap_signal":
      return <dl className="profile-fields">
        <div><dt>טווח</dt><dd>{fieldValue(s.startDate)} - {fieldValue(s.endDate)}</dd></div>
        <div><dt>סיבת האיתות</dt><dd>{fieldValue(s.signalReason)}</dd></div>
      </dl>;
    case "medical_missing_evidence_signal":
      return <dl className="profile-fields">
        <div><dt>סוג</dt><dd>{fieldValue(s.missingType)}</dd></div>
        <div><dt>תיאור</dt><dd>לא נמצא בחומר שנקלט: {fieldValue(s.description)}</dd></div>
      </dl>;
    case "medical_contradiction":
      return <dl className="profile-fields">
        <div><dt>צד א׳</dt><dd>{fieldValue(s.itemA)}</dd></div>
        <div><dt>צד ב׳</dt><dd>{fieldValue(s.itemB)}</dd></div>
        <div><dt>סיבת הסתירה</dt><dd>{fieldValue(s.reason)}</dd></div>
      </dl>;
    default:
      return null;
  }
}

function proposalTone(status: string): "ok" | "risk" | "warn" {
  if (status === "approved") return "ok";
  if (status === "rejected") return "risk";
  return "warn";
}

export function MedicalEvidenceTab({matterId}:{matterId:string}) {
  const {data:profiles} = useCommand(() => commands.get_ai_settings() as Promise<AiProfile[]>, []);
  const {data:proposals,loading,error,reload} = useCommand(
    () => commands.list_ai_proposals({matterId}) as Promise<AiProposal[]>, [matterId]
  );
  const enabledProfiles = profiles?.filter(p => p.enabled) ?? [];

  const [profileId,setProfileId] = useState("");
  const [query,setQuery] = useState("");
  const [egressApproved,setEgressApproved] = useState(false);
  const [busy,setBusy] = useState(false);
  const [runError,setRunError] = useState<string|null>(null);
  const [lastRunId,setLastRunId] = useState<string|null>(null);
  const [reviewingId,setReviewingId] = useState<string|null>(null);

  const selectedProfile = enabledProfiles.find(p => p.id === profileId);

  const medicalProposals = (proposals ?? []).filter(p => p.proposalKind.startsWith("medical_"));
  const bySection = new Map<string, AiProposal[]>();
  for (const kind of SECTION_ORDER) bySection.set(kind, []);
  for (const p of medicalProposals) bySection.get(p.proposalKind)?.push(p);

  const isFirstRun = medicalProposals.length === 0;
  const runButtonLabel = busy ? "מריץ..." : isFirstRun ? "בניית תמונה רפואית מחומר קיים" : "עדכון התמונה הרפואית";

  const runAi = async () => {
    if (!profileId) return;
    setBusy(true); setRunError(null); setLastRunId(null);
    try {
      const res = await commands.run_ai_capability({
        matterId, capability: CAPABILITY, profileId, externalEgressApproved: egressApproved,
        query: query.trim() || undefined,
      }) as {runId:string};
      setLastRunId(res.runId);
      reload();
    } catch (e) { setRunError(String(e)); }
    finally { setBusy(false); }
  };

  const review = async (proposalId:string, decision:"approved"|"rejected") => {
    setReviewingId(proposalId);
    try { await commands.review_ai_proposal({proposalId,decision}); reload(); }
    catch (e) { setRunError(String(e)); }
    finally { setReviewingId(null); }
  };

  return <div className="grid-2">
    <section className="workspace-card">
      <span className="eyebrow">AI · MEDICAL EVIDENCE INTELLIGENCE</span>
      <h2>ראיות רפואיות</h2>
      <p className="quiet">
        AI מזהה במסמכים בלבד: ביקורים, תלונות, ממצאים, אבחנות (עם רמת הוודאות כפי שנרשמה),
        בדיקות, טיפולים, תרופות, הפניות, תפקוד, קביעות נכות (רק מגורם מוסמך), היסטוריה קודמת וחוות דעת.
        המערכת אינה מאבחנת, אינה קובעת קשר סיבתי, ואינה מחשבת אחוזי נכות. כל פריט דורש אישור נפרד.
      </p>
      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        <label>מיקוד לחיפוש (אופציונלי)<input type="text" value={query} onChange={e=>setQuery(e.target.value)} placeholder="לדוגמה: אורתופדיה, MRI, נכות"/></label>
        {selectedProfile?.providerKind==="openai" && <label style={{display:"flex",alignItems:"center",gap:8,flexDirection:"row"}}>
          <input type="checkbox" checked={egressApproved} onChange={e=>setEgressApproved(e.target.checked)}/>
          מאשר שליחת חומר התיק החוצה להרצה זו
        </label>}
        <button className="btn primary" onClick={runAi} disabled={busy||!profileId}>{runButtonLabel}</button>
      </>}
      {runError && <p className="quiet">שגיאה: {runError}</p>}
      {lastRunId && !runError && <p className="quiet">הרצה הושלמה · {lastRunId.slice(0,12)}</p>}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span>
      <h2>הצעות לבדיקה</h2>
      {loading && <p className="quiet">טוען הצעות...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && medicalProposals.length===0 && <p className="quiet">לא זוהו עדיין ראיות רפואיות בתיק זה.</p>}
      {SECTION_ORDER.map(kind => {
        const items = bySection.get(kind) ?? [];
        if (items.length===0) return null;
        return <div key={kind} style={{marginTop:18}}>
          <h3>{SECTION_LABELS[kind]} <span className="quiet">({items.length})</span></h3>
          {items.map(p => <div className="proposal" key={p.id}>
            <div className="header-actions">
              <strong>{SECTION_LABELS[p.proposalKind]}</strong>
              <StatusBadge tone={proposalTone(p.status)}>{p.status}</StatusBadge>
            </div>
            <ItemPreview proposal={p}/>
            {typeof p.structured.confidence === "number" && <small className="quiet">רמת ביטחון של המודל: {(p.structured.confidence*100).toFixed(0)}%</small>}
            <small className="quiet"> · מבוסס על {p.structured.sourceIds?.length??0} מקור/ות</small>
            {p.sourceExcerpts.length>0 && <div className="source-excerpts">
              {p.sourceExcerpts.map(s=><blockquote key={s.sourceId} className="source-excerpt">
                <small className="quiet">{s.fileName??"מקור לא ידוע"}{s.page?` · עמוד ${s.page}`:""}</small>
                <p>{s.excerpt}{s.truncated?"…":""}</p>
              </blockquote>)}
            </div>}
            {p.status==="pending" && <div className="proposal-actions">
              <button className="primary-lite" disabled={reviewingId===p.id} onClick={()=>review(p.id,"approved")}>אשר</button>
              <button disabled={reviewingId===p.id} onClick={()=>review(p.id,"rejected")}>דחה</button>
            </div>}
            {p.status!=="pending" && p.reviewNote && <p className="quiet">{p.reviewNote}</p>}
          </div>)}
        </div>;
      })}
    </section>
  </div>;
}
