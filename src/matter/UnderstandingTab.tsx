import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiProposal } from "../types";

const CAPABILITY = "extract_matter_understanding";

const SECTION_ORDER = [
  "understanding_event", "understanding_date", "understanding_entity",
  "understanding_claim", "understanding_amount", "understanding_contradiction", "understanding_question",
] as const;

const SECTION_LABELS: Record<string, string> = {
  understanding_event: "אירועים",
  understanding_date: "תאריכים",
  understanding_entity: "ישויות",
  understanding_claim: "טענות",
  understanding_amount: "סכומים",
  understanding_contradiction: "סתירות אפשריות",
  understanding_question: "שאלות מוצעות לבדיקה",
};

function fieldValue(value?: string | number | null) {
  if (value === null || value === undefined || value === "") return "לא ידוע";
  return String(value);
}

function formatMoney(cents?: number) {
  if (typeof cents !== "number") return "לא צוין";
  return (cents / 100).toLocaleString("he-IL", {style: "currency", currency: "ILS"});
}

function proposalTone(status: string): "ok" | "risk" | "warn" {
  if (status === "approved") return "ok";
  if (status === "rejected") return "risk";
  return "warn";
}

function ItemPreview({proposal}:{proposal:AiProposal}) {
  const s = proposal.structured;
  switch (proposal.proposalKind) {
    case "understanding_entity":
      return <dl className="profile-fields">
        <div><dt>סוג</dt><dd>{fieldValue(s.entityType)}</dd></div>
        <div><dt>שם</dt><dd>{fieldValue(s.displayName)}</dd></div>
      </dl>;
    case "understanding_event":
      return <dl className="profile-fields">
        <div><dt>סוג אירוע</dt><dd>{fieldValue(s.eventType)}</dd></div>
        <div><dt>כותרת</dt><dd>{fieldValue(s.title)}</dd></div>
        <div><dt>תיאור</dt><dd>{fieldValue(s.description)}</dd></div>
        <div><dt>תאריך</dt><dd>{fieldValue(s.eventDate)}</dd></div>
        {s.involvedEntities && s.involvedEntities.length > 0 && <div><dt>גורמים מעורבים</dt><dd>{s.involvedEntities.join(", ")}</dd></div>}
      </dl>;
    case "understanding_claim":
      return <dl className="profile-fields">
        <div><dt>נטען ע״י</dt><dd>{fieldValue(s.assertedBy)}</dd></div>
        <div><dt>הטענה</dt><dd>{fieldValue(s.statement)}</dd></div>
        {s.target && <div><dt>נוגע ל</dt><dd>{s.target}</dd></div>}
      </dl>;
    case "understanding_amount":
      return <dl className="profile-fields">
        <div><dt>סוג</dt><dd>{fieldValue(s.amountType)}</dd></div>
        <div><dt>סכום</dt><dd>{formatMoney(s.amountCents)} {s.currency && s.currency !== "ILS" ? `(${s.currency})` : ""}</dd></div>
        {s.context && <div><dt>הקשר</dt><dd>{s.context}</dd></div>}
        <div><dt>תאריך</dt><dd>{fieldValue(s.eventDate)}</dd></div>
      </dl>;
    case "understanding_date":
      return <dl className="profile-fields">
        <div><dt>תאריך</dt><dd>{fieldValue(s.date)}</dd></div>
        <div><dt>סוג תאריך</dt><dd>{fieldValue(s.dateType)}</dd></div>
        <div><dt>הקשר</dt><dd>{fieldValue(s.context)}</dd></div>
      </dl>;
    case "understanding_contradiction":
      return <dl className="profile-fields">
        <div><dt>צד א׳</dt><dd>{fieldValue(s.itemA)}</dd></div>
        <div><dt>צד ב׳</dt><dd>{fieldValue(s.itemB)}</dd></div>
        <div><dt>סיבת הסתירה</dt><dd>{fieldValue(s.reason)}</dd></div>
      </dl>;
    case "understanding_question":
      return <p>{fieldValue(s.question)}</p>;
    default:
      return null;
  }
}

export function UnderstandingTab({matterId}:{matterId:string}) {
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

  const understandingProposals = (proposals ?? []).filter(p => p.proposalKind.startsWith("understanding_"));
  const bySection = new Map<string, AiProposal[]>();
  for (const kind of SECTION_ORDER) bySection.set(kind, []);
  for (const p of understandingProposals) bySection.get(p.proposalKind)?.push(p);

  return <div className="grid-2">
    <section className="workspace-card">
      <span className="eyebrow">AI · MATTER UNDERSTANDING</span>
      <h2>הבנת התיק</h2>
      <p className="quiet">
        AI מציע בלבד: ישויות, אירועים, טענות, סכומים, תאריכים וסתירות אפשריות מתוך המסמכים שנקלטו.
        כל הצעה דורשת אישור נפרד של עורך הדין ואינה משנה שום נתון בתיק לפני האישור.
      </p>
      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        <label>מיקוד לחיפוש (אופציונלי)<input type="text" value={query} onChange={e=>setQuery(e.target.value)} placeholder="לדוגמה: תאונה, ביטוח, שכר"/></label>
        {selectedProfile?.providerKind==="openai" && <label style={{display:"flex",alignItems:"center",gap:8,flexDirection:"row"}}>
          <input type="checkbox" checked={egressApproved} onChange={e=>setEgressApproved(e.target.checked)}/>
          מאשר שליחת חומר התיק החוצה להרצה זו
        </label>}
        <button className="btn primary" onClick={runAi} disabled={busy||!profileId}>
          {busy?"מריץ...":"סרוק והבן את התיק"}
        </button>
      </>}
      {runError && <p className="quiet">שגיאה: {runError}</p>}
      {lastRunId && !runError && <p className="quiet">הרצה הושלמה · {lastRunId.slice(0,12)}</p>}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span>
      <h2>הצעות לבדיקה</h2>
      {loading && <p className="quiet">טוען הצעות...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && understandingProposals.length===0 && <p className="quiet">אין עדיין הצעות הבנת-תיק בתיק זה.</p>}
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
