import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { loadMatterActionPlan } from "../lib/actionCenter";
import { ActionCandidateRow } from "../components/ActionCandidateRow";

type HealthFactor = {
  code:string; severity:"critical"|"high"|"attention"; count:number; penalty:number;
};

type CaseHealth = {
  matterId:string; score:number; band:"good"|"attention"|"risk"; asOf:string;
  factors:HealthFactor[];
};

const FACTOR_LABELS:Record<string,string> = {
  overdue_committed_deadlines:"מועדים מחייבים שעברו",
  due_soon_committed_deadlines:"מועדים מחייבים בשבעת הימים הקרובים",
  unresolved_fact_conflicts:"סתירות פתוחות בין עובדות מאומתות",
  overdue_tasks:"משימות פתוחות באיחור",
  blocked_workstreams:"מסלולי עבודה חסומים",
  required_evidence_stale:"ראיות שנדרש לרענן לפי מדיניות המשרד",
  required_evidence_missing:"ראיות חסרות לפי מדיניות המשרד",
  negotiation_followups_overdue:"מעקבי משא ומתן תפעוליים שעבר מועד המעקב שלהם",
  waiting_followups_overdue:"מעקבים מול גורם חיצוני שעבר מועד המעקב שלהם",
  stale_verified_ledgers:"רשומות מאומתות בפנקסים שהתיישנו",
  stale_verified_facts:"עובדות מאומתות שהתיישנו",
  documents_needing_attention:"מסמכים שחילוץ הטקסט שלהם חסום או מיושן",
  required_evidence_requested:"ראיות נדרשות שכבר התבקשו וטרם נאספו",
  recommended_evidence_open:"ראיות מומלצות שעדיין פתוחות",
  draft_deadlines_waiting_review:"מועדים בטיוטה שממתינים לאישור",
  ledger_drafts_waiting_review:"טיוטות פנקס שממתינות לבדיקה",
  pending_ai_review:"הצעות AI שממתינות לבדיקת עורך דין",
};

function bandLabel(band:CaseHealth["band"]){
  if(band==="good")return "תקין תפעולית";
  if(band==="attention")return "דורש תשומת לב";
  return "סיכון תפעולי";
}

// Phase C, milestone C5: the "next best action" here is no longer computed
// independently by this panel - it is the same Matter Action Plan's primary
// action that Today and the Action Center use, from action_engine.rs.
export function CaseHealthPanel({matterId}:{matterId:string}){
  const {data,loading,error}=useCommand(
    ()=>commands.get_case_health({matterId}) as Promise<CaseHealth>, [matterId]
  );
  const {data:plan,loading:planLoading,error:planError,reload:reloadPlan}=useCommand(
    ()=>loadMatterActionPlan(matterId), [matterId]
  );
  if(loading)return <section className="workspace-card"><h2>בריאות תיק</h2><p className="quiet">מחשב מצב תפעולי...</p></section>;
  if(error||!data)return <section className="workspace-card"><h2>בריאות תיק</h2><p className="quiet">לא ניתן לחשב כרגע: {error??"שגיאה לא ידועה"}</p></section>;
  return <div className="grid-2">
    <section className="workspace-card">
      <div className="card-head">
        <div><span className="eyebrow">CASE HEALTH</span><h2>בריאות תיק</h2></div>
        <strong>{data.score}/100</strong>
      </div>
      <p><strong>{bandLabel(data.band)}</strong></p>
      <p className="quiet">ציון תפעולי מחושב בלבד — לא הערכת סיכויי תיק ולא קביעה משפטית.</p>
      {data.factors.length===0
        ? <p className="quiet">לא נמצאו כרגע אותות תפעוליים שמפחיתים את הציון.</p>
        : data.factors.slice(0,5).map(f=><div className="mini-row" key={f.code}>
            <span>{FACTOR_LABELS[f.code]??f.code}</span><small>{f.count} · ‎-{f.penalty}</small>
          </div>)}
    </section>
    <section className="workspace-card">
      <span className="eyebrow">NEXT BEST ACTION</span><h2>הפעולה הבאה</h2>
      {planLoading && <p className="quiet">מחשב...</p>}
      {planError && <p className="quiet">לא ניתן לחשב כרגע: {planError}</p>}
      {plan && !plan.primaryAction && <p className="quiet">לא נמצא כרגע אות תפעולי דחוף.</p>}
      {plan?.primaryAction && <ActionCandidateRow candidate={plan.primaryAction} onChanged={reloadPlan}/>}
      {plan && plan.alternatives.length>0 && <div style={{marginTop:10}}>
        <span className="eyebrow">חלופות</span>
        {plan.alternatives.map(a=><ActionCandidateRow key={a.fingerprint} candidate={a} onChanged={reloadPlan}/>)}
      </div>}
      <p className="quiet">העדיפות נגזרת רק ממצב התיק הקיים; TAHRIR אינה מבצעת את הפעולה או מאשרת אותה בעצמה.</p>
    </section>
  </div>;
}
