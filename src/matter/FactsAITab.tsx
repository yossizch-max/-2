import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { AiProfile, AiRun, VerifiedFact } from "../types";

const CAPABILITY = "extract_facts";

export function FactsAITab({matterId}:{matterId:string}) {
  const {data:facts,loading,error,reload}=useCommand(
    ()=>commands.list_verified_facts({matterId}) as Promise<VerifiedFact[]>, [matterId]
  );
  const {data:profiles}=useCommand(
    ()=>commands.get_ai_settings() as Promise<AiProfile[]>, []
  );
  const enabledProfiles=profiles?.filter(p=>p.enabled)??[];

  const [profileId,setProfileId]=useState("");
  const [egressApproved,setEgressApproved]=useState(false);
  const [runId,setRunId]=useState<string|null>(null);
  const [busy,setBusy]=useState(false);
  const [runError,setRunError]=useState<string|null>(null);
  const [reviewingId,setReviewingId]=useState<string|null>(null);

  const {data:run,loading:runLoading,error:runFetchError,reload:reloadRun}=useCommand(
    ()=>runId ? commands.get_ai_run({runId}) as Promise<AiRun> : Promise.resolve(undefined),
    [runId]
  );

  const selectedProfile=enabledProfiles.find(p=>p.id===profileId);

  const invalidate=async(factId:string)=>{
    await commands.invalidate_fact({factId});
    reload();
  };

  const runAi=async()=>{
    if(!profileId)return;
    setBusy(true);setRunError(null);
    try{
      const res=await commands.run_ai_capability({
        matterId, capability:CAPABILITY, profileId, externalEgressApproved:egressApproved
      }) as {runId:string};
      setRunId(res.runId);
    }catch(e){ setRunError(String(e)); }
    finally{ setBusy(false); }
  };

  const review=async(proposalId:string,decision:"approved"|"rejected"|"needs_revision")=>{
    setReviewingId(proposalId);setRunError(null);
    try{
      await commands.review_ai_proposal({proposalId,decision});
      reloadRun();
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
        <button className="source-link">פתח מקור · {f.sourceLabel}</button>
        <button className="source-link" onClick={()=>invalidate(f.id)}>בטל תוקף</button>
      </div>)}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">AI REVIEW</span><h2>הצעות לבדיקה</h2>
      <p className="quiet">AI אינו כותב עובדה מאומתת ישירות — הוא מציע, ועורך הדין מאשר או דוחה כל הצעה בנפרד.</p>

      {enabledProfiles.length===0 && <p className="quiet">אין ספק AI פעיל. יש להגדיר ולהפעיל ספק בעמוד ה-AI תחילה.</p>}
      {enabledProfiles.length>0 && <>
        <label>ספק<select value={profileId} onChange={e=>setProfileId(e.target.value)}>
          <option value="">בחר ספק...</option>
          {enabledProfiles.map(p=><option key={p.id} value={p.id}>{p.providerKind} · {p.model||"—"}</option>)}
        </select></label>
        {selectedProfile?.providerKind==="openai" && <label style={{display:"flex",alignItems:"center",gap:8,flexDirection:"row"}}>
          <input type="checkbox" checked={egressApproved} onChange={e=>setEgressApproved(e.target.checked)}/>
          מאשר שליחת חומר התיק החוצה להרצה זו
        </label>}
        <button className="btn primary" onClick={runAi} disabled={busy||!profileId}>
          {busy?"מריץ...":"הפעל בדיקת AI על עובדות התיק"}
        </button>
      </>}

      {runError && <p className="quiet">שגיאה: {runError}</p>}
      {runId && runLoading && <p className="quiet">טוען הרצה...</p>}
      {runId && runFetchError && <p className="quiet">שגיאה: {runFetchError}</p>}
      {run && <div style={{marginTop:14}}>
        <div className="header-actions">
          <StatusBadge tone={run.status==="completed"?"ok":run.status==="failed"?"risk":"warn"}>{run.status}</StatusBadge>
          <span className="quiet">{run.proposals.length} הצעות</span>
        </div>
        {run.proposals.length===0 && run.status==="completed" &&
          <p className="quiet">ה-AI לא הציע עובדות מהמקורות הזמינים בתיק זה.</p>}
        {run.proposals.map(p=><div className="proposal" key={p.id}>
          <p><strong>{p.structured.subject} · {p.structured.predicate}</strong>: {p.structured.value}</p>
          <small className="quiet">מבוסס על {p.structured.sourceIds?.length??0} מקור/ות · {p.status}</small>
          {p.status==="pending" && <div className="proposal-actions">
            <button className="primary-lite" disabled={reviewingId===p.id} onClick={()=>review(p.id,"approved")}>אשר → צור עובדה מאומתת</button>
            <button disabled={reviewingId===p.id} onClick={()=>review(p.id,"needs_revision")}>דורש תיקון</button>
            <button disabled={reviewingId===p.id} onClick={()=>review(p.id,"rejected")}>דחה</button>
          </div>}
          {p.status!=="pending" && <p className="quiet">{p.reviewNote}</p>}
        </div>)}
      </div>}
    </section>
  </div>;
}
