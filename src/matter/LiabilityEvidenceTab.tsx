import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiProposal } from "../types";

const CAPABILITY = "extract_liability_evidence";

const SECTION_ORDER = [
  "liability_version_statement", "liability_witness_statement", "liability_scene_evidence",
  "liability_police_evidence", "liability_vehicle_damage", "liability_photo_video_evidence",
  "liability_expert_opinion", "liability_admission", "liability_insurer_position",
  "liability_court_finding", "liability_contradiction",
] as const;

const SECTION_LABELS: Record<string, string> = {
  liability_version_statement: "גרסאות הצדדים",
  liability_witness_statement: "עדויות",
  liability_scene_evidence: "ראיות זירה אובייקטיביות",
  liability_police_evidence: "חומר משטרתי",
  liability_vehicle_damage: "נזק לרכב",
  liability_photo_video_evidence: "תמונות/סרטונים",
  liability_expert_opinion: "חוות דעת מומחה",
  liability_admission: "הודאות",
  liability_insurer_position: "עמדת המבטח",
  liability_court_finding: "קביעות בית משפט",
  liability_contradiction: "סתירות אפשריות",
};

function fieldValue(value?: string | number | null) {
  if (value === null || value === undefined || value === "") return "לא ידוע";
  return String(value);
}

function ItemPreview({proposal}:{proposal:AiProposal}) {
  const s = proposal.structured;
  switch (proposal.proposalKind) {
    case "liability_version_statement":
      return <dl className="profile-fields">
        <div><dt>נטען ע״י</dt><dd>{fieldValue(s.assertedBy)}</dd></div>
        <div><dt>גרסה (טענה, לא עובדה)</dt><dd>{fieldValue(s.statement)}</dd></div>
        {s.issue && <div><dt>סוגיה</dt><dd>{s.issue}</dd></div>}
      </dl>;
    case "liability_witness_statement":
      return <dl className="profile-fields">
        <div><dt>עד/ה</dt><dd>{fieldValue(s.witness)}</dd></div>
        <div><dt>עדות (טענה, לא עובדה)</dt><dd>{fieldValue(s.statement)}</dd></div>
        {s.issue && <div><dt>סוגיה</dt><dd>{s.issue}</dd></div>}
      </dl>;
    case "liability_scene_evidence":
      return <dl className="profile-fields">
        <div><dt>סוג ראיה</dt><dd>{fieldValue(s.evidenceType)}</dd></div>
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
      </dl>;
    case "liability_police_evidence":
      return <dl className="profile-fields">
        <div><dt>סוג דוח</dt><dd>{fieldValue(s.reportType)}</dd></div>
        <div><dt>תוכן עובדתי</dt><dd>{fieldValue(s.factualContent)}</dd></div>
      </dl>;
    case "liability_vehicle_damage":
      return <dl className="profile-fields">
        <div><dt>רכב</dt><dd>{fieldValue(s.vehicle)}</dd></div>
        <div><dt>מיקום הנזק</dt><dd>{fieldValue(s.damageLocation)}</dd></div>
        <div><dt>מצב מתועד</dt><dd>{fieldValue(s.documentedCondition)}</dd></div>
      </dl>;
    case "liability_photo_video_evidence":
      return <dl className="profile-fields">
        <div><dt>סוג מדיה</dt><dd>{fieldValue(s.mediaType)}</dd></div>
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
      </dl>;
    case "liability_expert_opinion":
      return <dl className="profile-fields">
        <div><dt>מומחה</dt><dd>{fieldValue(s.expert)}</dd></div>
        <div><dt>תחום</dt><dd>{fieldValue(s.specialty)}</dd></div>
        <div><dt>חוות דעת</dt><dd>{fieldValue(s.opinionText)}</dd></div>
      </dl>;
    case "liability_admission":
      return <dl className="profile-fields">
        <div><dt>נטען ע״י</dt><dd>{fieldValue(s.assertedBy)}</dd></div>
        <div><dt>נוסח ההודאה</dt><dd>{fieldValue(s.statement)}</dd></div>
      </dl>;
    case "liability_insurer_position":
      return <dl className="profile-fields">
        <div><dt>עמדה</dt><dd>{fieldValue(s.position)}</dd></div>
        {s.detail && <div><dt>פירוט</dt><dd>{s.detail}</dd></div>}
        {s.insurer && <div><dt>מבטח</dt><dd>{s.insurer}</dd></div>}
      </dl>;
    case "liability_court_finding":
      return <dl className="profile-fields">
        <div><dt>סוג הקביעה</dt><dd>{fieldValue(s.findingType)}</dd></div>
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
      </dl>;
    case "liability_contradiction":
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

export function LiabilityEvidenceTab({matterId}:{matterId:string}) {
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

  const liabilityProposals = (proposals ?? []).filter(p => p.proposalKind.startsWith("liability_"));
  const bySection = new Map<string, AiProposal[]>();
  for (const kind of SECTION_ORDER) bySection.set(kind, []);
  for (const p of liabilityProposals) bySection.get(p.proposalKind)?.push(p);

  const isFirstRun = liabilityProposals.length === 0;
  const runButtonLabel = busy ? "מריץ..." : isFirstRun ? "בניית תמונת אחריות מחומר קיים" : "עדכון תמונת האחריות";

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
      <span className="eyebrow">AI · LIABILITY EVIDENCE INTELLIGENCE</span>
      <h2>ראיות אחריות</h2>
      <p className="quiet">
        AI מזהה במסמכים בלבד: גרסאות צדדים ועדויות (נשארות טענות, לא עובדות), ראיות זירה אובייקטיביות,
        חומר משטרתי, נזק לרכב, תמונות/סרטונים, חוות דעת מומחה, הודאות (רק כשהלשון תומכת בכך במפורש),
        עמדת מבטח, וקביעות בית משפט (תוך שמירה על ההבחנה בין הערת ביניים לקביעה עובדתית לפסק דין סופי).
        המערכת אינה קובעת אשם, רשלנות, אחוז רשלנות תורמת, או אמינות עדים. כל פריט דורש אישור נפרד.
      </p>
      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        <label>מיקוד לחיפוש (אופציונלי)<input type="text" value={query} onChange={e=>setQuery(e.target.value)} placeholder="לדוגמה: משטרה, עד, רמזור"/></label>
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
      {!loading && !error && liabilityProposals.length===0 && <p className="quiet">לא זוהו עדיין ראיות אחריות בתיק זה.</p>}
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
