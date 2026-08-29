import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiProposal } from "../types";

const CAPABILITY = "extract_wage_economic_evidence";

const SECTION_ORDER = [
  "wage_employment", "wage_income", "wage_payslip", "wage_annual_income",
  "wage_employer_confirmation", "wage_self_employed_income", "wage_pension_contribution",
  "wage_absence", "wage_sick_leave", "wage_work_limitation",
  "wage_employment_change", "wage_benefit_payment", "wage_gap_signal",
] as const;

const SECTION_LABELS: Record<string, string> = {
  wage_employment: "העסקה",
  wage_income: "הכנסה",
  wage_payslip: "תלושי שכר",
  wage_annual_income: "הכנסה שנתית של שכיר (טופס 106 וכו׳)",
  wage_employer_confirmation: "אישורי מעסיק",
  wage_self_employed_income: "הכנסה כעצמאי",
  wage_pension_contribution: "הפרשות פנסיוניות/סוציאליות",
  wage_absence: "היעדרויות מהעבודה",
  wage_sick_leave: "תעודות מחלה",
  wage_work_limitation: "מגבלות עבודה",
  wage_employment_change: "שינויים תעסוקתיים",
  wage_benefit_payment: "תשלומים/גמלאות",
  wage_gap_signal: "פערי תיעוד כלכליים אפשריים",
};

function fieldValue(value?: string | number | null) {
  if (value === null || value === undefined || value === "") return "לא ידוע";
  return String(value);
}

function centsToILS(value?: number | null) {
  if (typeof value !== "number") return "לא ידוע";
  return `${(value / 100).toFixed(2)} ש"ח`;
}

function ItemPreview({proposal}:{proposal:AiProposal}) {
  const s = proposal.structured;
  switch (proposal.proposalKind) {
    case "wage_employment":
      return <dl className="profile-fields">
        <div><dt>מעסיק</dt><dd>{fieldValue(s.employer)}</dd></div>
        <div><dt>תפקיד</dt><dd>{fieldValue(s.role)}</dd></div>
        <div><dt>סטטוס תעסוקתי</dt><dd>{fieldValue(s.employmentStatus)}</dd></div>
        <div><dt>תקופה</dt><dd>{fieldValue(s.startDate)} - {fieldValue(s.endDate)}</dd></div>
      </dl>;
    case "wage_income":
      return <dl className="profile-fields">
        <div><dt>סכום</dt><dd>{centsToILS(s.amountCents)} ({fieldValue(s.amountBasis)})</dd></div>
        <div><dt>סוג הכנסה</dt><dd>{fieldValue(s.incomeType)}</dd></div>
        {s.employerOrSource && <div><dt>מקור</dt><dd>{s.employerOrSource}</dd></div>}
      </dl>;
    case "wage_payslip":
      return <dl className="profile-fields">
        <div><dt>חודש</dt><dd>{fieldValue(s.month)}</dd></div>
        <div><dt>ברוטו</dt><dd>{centsToILS(s.grossAmountCents)}</dd></div>
        <div><dt>נטו</dt><dd>{centsToILS(s.netAmountCents)}</dd></div>
      </dl>;
    case "wage_annual_income":
      return <dl className="profile-fields">
        <div><dt>מקור המסמך</dt><dd>{fieldValue(s.sourceType)}</dd></div>
        <div><dt>שנה</dt><dd>{fieldValue(s.year)}</dd></div>
        <div><dt>סכום</dt><dd>{centsToILS(s.amountCents)}</dd></div>
        {typeof s.monthsWorked === "number" && <div><dt>חודשי עבודה</dt><dd>{s.monthsWorked}</dd></div>}
      </dl>;
    case "wage_employer_confirmation":
      return <dl className="profile-fields">
        <div><dt>מעסיק</dt><dd>{fieldValue(s.employer)}</dd></div>
        <div><dt>תקופה</dt><dd>{fieldValue(s.periodStart)} - {fieldValue(s.periodEnd)}</dd></div>
        <div><dt>שכר כפי שנרשם ע״י המעסיק</dt><dd>{fieldValue(s.statedSalaryText)}</dd></div>
        {s.terminationReasonStated && <div><dt>סיבת סיום כפי שנרשמה</dt><dd>{s.terminationReasonStated}</dd></div>}
      </dl>;
    case "wage_self_employed_income":
      return <dl className="profile-fields">
        <div><dt>סוג מסמך</dt><dd>{fieldValue(s.documentType)}</dd></div>
        <div><dt>שנת מס</dt><dd>{fieldValue(s.taxYear)}</dd></div>
        <div><dt>הכנסות</dt><dd>{centsToILS(s.revenueCents)}</dd></div>
        <div><dt>הוצאות</dt><dd>{centsToILS(s.expensesCents)}</dd></div>
        <div><dt>רווח/הכנסה חייבת</dt><dd>{centsToILS(s.profitCents)}</dd></div>
      </dl>;
    case "wage_pension_contribution":
      return <dl className="profile-fields">
        <div><dt>הפרשת מעסיק</dt><dd>{centsToILS(s.employerContributionCents)}</dd></div>
        <div><dt>הפרשת עובד</dt><dd>{centsToILS(s.employeeContributionCents)}</dd></div>
        {s.pensionComponent && <div><dt>רכיב</dt><dd>{s.pensionComponent}</dd></div>}
        {s.trainingFund && <div><dt>קרן השתלמות</dt><dd>{s.trainingFund}</dd></div>}
      </dl>;
    case "wage_absence":
      return <dl className="profile-fields">
        <div><dt>תקופה</dt><dd>{fieldValue(s.startDate)} - {fieldValue(s.endDate)}</dd></div>
        <div><dt>סיבה כפי שדווחה</dt><dd>{fieldValue(s.statedReason)}</dd></div>
      </dl>;
    case "wage_sick_leave":
      return <dl className="profile-fields">
        <div><dt>תקופה</dt><dd>{fieldValue(s.startDate)} - {fieldValue(s.endDate)}</dd></div>
        <div><dt>גורם מנפיק</dt><dd>{fieldValue(s.issuingSource)}</dd></div>
      </dl>;
    case "wage_work_limitation":
      return <dl className="profile-fields">
        <div><dt>מגבלה</dt><dd>{fieldValue(s.limitation)}</dd></div>
        <div><dt>סטטוס כושר עבודה</dt><dd>{fieldValue(s.workCapacityStatus)}</dd></div>
      </dl>;
    case "wage_employment_change":
      return <dl className="profile-fields">
        <div><dt>סוג שינוי</dt><dd>{fieldValue(s.changeType)}</dd></div>
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
      </dl>;
    case "wage_benefit_payment":
      return <dl className="profile-fields">
        <div><dt>סוג תשלום</dt><dd>{fieldValue(s.paymentType)}</dd></div>
        <div><dt>סכום</dt><dd>{centsToILS(s.amountCents)}</dd></div>
        {s.payer && <div><dt>משלם</dt><dd>{s.payer}</dd></div>}
      </dl>;
    case "wage_gap_signal":
      return <dl className="profile-fields">
        <div><dt>סוג</dt><dd>{fieldValue(s.gapType)}</dd></div>
        <div><dt>תיאור</dt><dd>לא נמצא בחומר שנקלט: {fieldValue(s.description)}</dd></div>
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

export function WageEvidenceTab({matterId}:{matterId:string}) {
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

  const wageProposals = (proposals ?? []).filter(p => p.proposalKind.startsWith("wage_"));
  const bySection = new Map<string, AiProposal[]>();
  for (const kind of SECTION_ORDER) bySection.set(kind, []);
  for (const p of wageProposals) bySection.get(p.proposalKind)?.push(p);

  const isFirstRun = wageProposals.length === 0;
  const runButtonLabel = busy ? "מריץ..." : isFirstRun ? "בניית תמונת שכר מחומר קיים" : "עדכון תמונת השכר";

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
      <span className="eyebrow">AI · WAGE / ECONOMIC EVIDENCE INTELLIGENCE</span>
      <h2>ראיות שכר וכלכליות</h2>
      <p className="quiet">
        AI מזהה במסמכים בלבד: העסקה, הכנסה (ברוטו/נטו כפי שנרשם, ללא המרה, כולל מקור התיעוד), תלושי שכר,
        הכנסה שנתית של שכיר (טופס 106 וכו׳), אישורי מעסיק, הכנסה כעצמאי (הכנסות/הוצאות/רווח - מושגים נפרדים),
        הפרשות פנסיוניות, היעדרויות, תעודות מחלה, מגבלות עבודה, שינויים תעסוקתיים, ותשלומים/גמלאות (בנפרד מהשכר).
        המערכת אינה מחשבת אובדן שכר בפועל, אובדן כושר השתכרות, היוון, או אובדן פנסיה, ואינה קובעת קשר סיבתי
        בין שינוי תעסוקתי או היעדרות לבין האירוע. סכומים סותרים ממקורות שונים מוצגים שניהם - המערכת אינה בוחרת
        ביניהם. כל פריט דורש אישור נפרד.
      </p>
      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        <label>מיקוד לחיפוש (אופציונלי)<input type="text" value={query} onChange={e=>setQuery(e.target.value)} placeholder="לדוגמה: תלוש שכר, טופס 106, מעסיק"/></label>
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
      {!loading && !error && wageProposals.length===0 && <p className="quiet">לא זוהו עדיין ראיות שכר בתיק זה.</p>}
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
