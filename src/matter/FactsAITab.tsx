import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiProposal, VerifiedFact } from "../types";

type Capability = "extract_facts" | "extract_medical_event" | "extract_wage_record" | "extract_liability_fact";

const CAPABILITIES: Array<{value: Capability; label: string; buttonLabel: string; placeholder: string}> = [
  {value: "extract_facts", label: "עובדה כללית", buttonLabel: "הצע עובדה לבדיקה", placeholder: "לדוגמה: שבר בכף היד"},
  {value: "extract_medical_event", label: "אירוע רפואי", buttonLabel: "הצע אירועים רפואיים", placeholder: "לדוגמה: אשפוז, אבחנה, טיפול"},
  {value: "extract_wage_record", label: "רשומת שכר", buttonLabel: "הצע רשומות שכר", placeholder: "לדוגמה: תלוש שכר, מעסיק, היעדרות"},
  {value: "extract_liability_fact", label: "עובדת חבות", buttonLabel: "הצע עובדות חבות", placeholder: "לדוגמה: עדות, דו\"ח משטרה, תאונה"},
];

const PROPOSAL_KIND_LABELS: Record<string, string> = {
  extract_facts: "עובדה כללית",
  extract_medical_event: "אירוע רפואי",
  extract_wage_record: "רשומת שכר",
  extract_liability_fact: "עובדת חבות",
};

function fieldValue(value?: string | number | null) {
  if (value === null || value === undefined || value === "") return "לא צוין";
  return String(value);
}

function formatMoney(cents?: number) {
  if (typeof cents !== "number") return "לא צוין";
  return (cents / 100).toLocaleString("he-IL", {style: "currency", currency: "ILS"});
}

function approvalButtonLabel(kind: string) {
  return kind === "extract_facts" ? "אשר → צור עובדה מאומתת" : "אשר → צור טיוטת פנקס";
}

function proposalTone(status: string): "ok" | "risk" | "warn" {
  if (status === "approved") return "ok";
  if (status === "rejected") return "risk";
  return "warn";
}

function StructuredPreview({proposal}:{proposal:AiProposal}) {
  const s = proposal.structured;
  if (proposal.proposalKind === "extract_medical_event") {
    return <dl className="profile-fields">
      <div><dt>תאריך</dt><dd>{fieldValue(s.eventDate)}</dd></div>
      <div><dt>גורם מטפל</dt><dd>{fieldValue(s.providerName)}</dd></div>
      <div><dt>תיאור טיפול</dt><dd>{fieldValue(s.treatmentSummary)}</dd></div>
    </dl>;
  }
  if (proposal.proposalKind === "extract_wage_record") {
    return <dl className="profile-fields">
      <div><dt>תקופה</dt><dd>{fieldValue(s.periodStart)} - {fieldValue(s.periodEnd)}</dd></div>
      <div><dt>מעסיק</dt><dd>{fieldValue(s.employerName)}</dd></div>
      <div><dt>שכר ברוטו</dt><dd>{formatMoney(s.grossAmountCents)}</dd></div>
    </dl>;
  }
  if (proposal.proposalKind === "extract_liability_fact") {
    return <dl className="profile-fields">
      <div><dt>בסיס</dt><dd>{fieldValue(s.claimBasis)}</dd></div>
      <div><dt>צד רלוונטי</dt><dd>{fieldValue(s.liablePartyName)}</dd></div>
      <div><dt>תיאור עובדתי</dt><dd>{fieldValue(s.description)}</dd></div>
    </dl>;
  }
  return <dl className="profile-fields">
    <div><dt>נושא</dt><dd>{fieldValue(s.subject)}</dd></div>
    <div><dt>יחס</dt><dd>{fieldValue(s.predicate)}</dd></div>
    <div><dt>ערך</dt><dd>{fieldValue(s.value)}</dd></div>
  </dl>;
}

export function FactsAITab({matterId}:{matterId:string}) {
  const {data:facts,loading,error,reload}=useCommand(
    ()=>commands.list_verified_facts({matterId}) as Promise<VerifiedFact[]>, [matterId]
  );
  const {data:profiles}=useCommand(
    ()=>commands.get_ai_settings() as Promise<AiProfile[]>, []
  );
  const {data:proposals,loading:queueLoading,error:queueError,reload:reloadQueue}=useCommand(
    ()=>commands.list_ai_proposals({matterId}) as Promise<AiProposal[]>, [matterId]
  );
  const enabledProfiles=profiles?.filter(p=>p.enabled)??[];

  const [capability,setCapability]=useState<Capability>("extract_facts");
  const [profileId,setProfileId]=useState("");
  const [query,setQuery]=useState("");
  const [egressApproved,setEgressApproved]=useState(false);
  const [busy,setBusy]=useState(false);
  const [runError,setRunError]=useState<string|null>(null);
  const [reviewingId,setReviewingId]=useState<string|null>(null);
  const [lastRunId,setLastRunId]=useState<string|null>(null);

  const selectedProfile=enabledProfiles.find(p=>p.id===profileId);
  const selectedCapability=CAPABILITIES.find(c=>c.value===capability)??CAPABILITIES[0];

  const invalidate=async(factId:string)=>{
    await commands.invalidate_fact({factId});
    reload();
  };

  const openSource=(occurrenceId:string)=>{
    commands.open_occurrence({occurrenceId});
  };

  const runAi=async()=>{
    if(!profileId)return;
    setBusy(true);setRunError(null);setLastRunId(null);
    try{
      const res=await commands.run_ai_capability({
        matterId, capability, profileId, externalEgressApproved:egressApproved,
        query: query.trim()||undefined
      }) as {runId:string};
      setLastRunId(res.runId);
      reloadQueue();
    }catch(e){ setRunError(String(e)); }
    finally{ setBusy(false); }
  };

  const review=async(proposalId:string,decision:"approved"|"rejected")=>{
    setReviewingId(proposalId);setRunError(null);
    try{
      await commands.review_ai_proposal({proposalId,decision});
      reloadQueue();
      if(decision==="approved")reload();
    }catch(e){ setRunError(String(e)); }
    finally{ setReviewingId(null); }
  };

  return <div className="grid-2">
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">VERIFIED LEDGER</span><h2>עובדות מאומתות</h2></div><StatusBadge tone="ok">Human approved</StatusBadge></div>
      {loading && <p className="quiet">טוען עובדות...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && facts?.length===0 && <p className="quiet">אין עדיין עובדות מאומתות בתיק זה.</p>}
      {facts?.map(f=><div className="fact-row" key={f.id}>
        <strong>{f.subject} · {f.predicate}</strong><p>{f.value}</p>
        <button className="source-link" disabled={!f.occurrenceId} onClick={()=>f.occurrenceId&&openSource(f.occurrenceId)}>פתח מקור · {f.sourceLabel}</button>
        <button className="source-link" onClick={()=>invalidate(f.id)}>בטל תוקף</button>
      </div>)}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span><h2>הצעות לבדיקה</h2>
      <p className="quiet">AI מציע בלבד; הצעות נשמרות בתיק עד שהן מאושרות או נדחות.</p>

      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>סוג הצעה<select value={capability} onChange={e=>setCapability(e.target.value as Capability)}>
          {CAPABILITIES.map(c=><option key={c.value} value={c.value}>{c.label}</option>)}
        </select></label>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        <label>מיקוד לחיפוש (אופציונלי)<input type="text" value={query} onChange={e=>setQuery(e.target.value)} placeholder={selectedCapability.placeholder}/></label>
        {selectedProfile?.providerKind==="openai" && <label style={{display:"flex",alignItems:"center",gap:8,flexDirection:"row"}}>
          <input type="checkbox" checked={egressApproved} onChange={e=>setEgressApproved(e.target.checked)}/>
          מאשר שליחת חומר התיק החוצה להרצה זו
        </label>}
        <button className="btn primary" onClick={runAi} disabled={busy||!profileId}>
          {busy?"מריץ...":selectedCapability.buttonLabel}
        </button>
      </>}

      {runError && <p className="quiet">שגיאה: {runError}</p>}
      {lastRunId && !runError && <p className="quiet">הרצת AI הושלמה · {lastRunId.slice(0,12)}</p>}
      {queueLoading && <p className="quiet">טוען תור בדיקה...</p>}
      {queueError && <p className="quiet">שגיאה בטעינת תור הבדיקה: {queueError}</p>}
      {!queueLoading && !queueError && proposals?.length===0 && <p className="quiet">אין עדיין הצעות AI בתיק זה.</p>}
      {proposals && <div style={{marginTop:14}}>
        <div className="header-actions"><span className="quiet">{proposals.length} הצעות שמורות בתיק</span></div>
        {proposals.map(p=><div className="proposal" key={p.id}>
          <div className="header-actions">
            <strong>{PROPOSAL_KIND_LABELS[p.proposalKind]??p.proposalKind}</strong>
            <StatusBadge tone={proposalTone(p.status)}>{p.status}</StatusBadge>
          </div>
          <StructuredPreview proposal={p}/>
          {p.structured.explanation && <p className="quiet">{p.structured.explanation}</p>}
          <small className="quiet">מבוסס על {p.structured.sourceIds?.length??0} מקור/ות</small>
          {p.sourceManifestSha256 && <small className="quiet"> · manifest {p.sourceManifestSha256.slice(0,12)}</small>}
          {p.sourceExcerpts.length>0 && <div className="source-excerpts">
            {p.sourceExcerpts.map(s=><blockquote key={s.sourceId} className="source-excerpt">
              <small className="quiet">{s.fileName??"מקור לא ידוע"}{s.page?` · עמוד ${s.page}`:""}</small>
              <p>{s.excerpt}{s.truncated?"…":""}</p>
            </blockquote>)}
          </div>}
          {p.status==="pending" && <div className="proposal-actions">
            <button className="primary-lite" disabled={reviewingId===p.id} onClick={()=>review(p.id,"approved")}>{approvalButtonLabel(p.proposalKind)}</button>
            <button disabled={reviewingId===p.id} onClick={()=>review(p.id,"rejected")}>דחה</button>
          </div>}
          {p.status!=="pending" && p.reviewNote && <p className="quiet">{p.reviewNote}</p>}
        </div>)}
      </div>}
    </section>
  </div>;
}
