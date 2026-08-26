import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { REQUIREMENT_KEYS, WORKSTREAM_KINDS } from "../types";

type HealthFactor = {
  code:string; severity:"critical"|"high"|"attention"; count:number; penalty:number;
};

type NextBestAction = {
  code:string; priority:"critical"|"high"|"normal";
  targetId?:string|null; dueAt?:string|null; label?:string|null; secondaryLabel?:string|null;
  requirementKey?:string|null; workstreamKind?:string|null;
};

type CaseHealth = {
  matterId:string; score:number; band:"good"|"attention"|"risk"; asOf:string;
  factors:HealthFactor[]; nextBestAction:NextBestAction;
};

const FACTOR_LABELS:Record<string,string> = {
  overdue_committed_deadlines:"מועדים מחייבים שעברו",
  due_soon_committed_deadlines:"מועדים מחייבים בשבעת הימים הקרובים",
  unresolved_fact_conflicts:"סתירות פתוחות בין עובדות מאומתות",
  overdue_tasks:"משימות פתוחות באיחור",
  blocked_workstreams:"מסלולי עבודה חסומים",
  required_evidence_stale:"ראיות שנדרש לרענן לפי מדיניות המשרד",
  required_evidence_missing:"ראיות חסרות לפי מדיניות המשרד",
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

function requirementLabel(key?:string|null){
  return REQUIREMENT_KEYS.find(x=>x.value===key)?.label ?? key ?? "ראיה";
}
function workstreamLabel(kind?:string|null){
  return WORKSTREAM_KINDS.find(x=>x.value===kind)?.label ?? kind ?? "מסלול עבודה";
}

function actionText(action:NextBestAction):{title:string;detail:string}{
  switch(action.code){
    case "resolve_overdue_deadline":
      return {title:`טפל מיד במועד: ${action.label??"מועד מחייב"}`, detail:action.dueAt?`המועד היה ${action.dueAt}`:"מועד מחייב שעבר"};
    case "prepare_upcoming_deadline":
      return {title:`הכן את הפעולה למועד: ${action.label??"מועד מחייב"}`, detail:action.dueAt?`יעד: ${action.dueAt}`:"מועד מחייב קרוב"};
    case "review_fact_conflict":
      return {title:"בדוק סתירה פתוחה בין עובדות מאומתות", detail:"נדרשת הכרעה אנושית לפני שימוש בעובדות הסותרות"};
    case "complete_overdue_task":
      return {title:action.label??"בצע משימה באיחור", detail:action.dueAt?`יעד שעבר: ${action.dueAt}`:"משימה פתוחה באיחור"};
    case "unblock_workstream":
      return {title:`פתח את החסימה במסלול ${workstreamLabel(action.workstreamKind)}`, detail:"המסלול מסומן כחסום"};
    case "refresh_required_evidence":
      return {title:`רענן: ${requirementLabel(action.requirementKey)}`, detail:"נדרש לפי מדיניות המשרד ומסומן כמיושן"};
    case "collect_required_evidence":
      return {title:`השג: ${requirementLabel(action.requirementKey)}`, detail:"חסר לפי מדיניות המשרד"};
    case "follow_up_waiting":
      return {title:`בצע מעקב מול ${action.label??"הגורם החיצוני"}`, detail:`ממתינים ל${action.secondaryLabel??"פריט"}${action.dueAt?` · מעקב: ${action.dueAt}`:""}`};
    case "refresh_stale_evidence":
      return {title:"רענן נתונים מאומתים שהתיישנו", detail:"יש עובדה או רשומת פנקס מאומתת שסומנה stale"};
    case "repair_document_extraction":
      return {title:"טפל במסמכים שחילוץ הטקסט שלהם דורש תשומת לב", detail:"יש מסמך blocked או stale"};
    case "review_ai_proposals":
      return {title:"בדוק את תור הצעות ה־AI", detail:"ההצעות נשמרו כטיוטות ואינן מאושרות אוטומטית"};
    case "complete_open_task":
      return {title:action.label??"בצע את המשימה הפתוחה הבאה", detail:action.dueAt?`יעד: ${action.dueAt}`:"משימה פתוחה"};
    case "follow_up_required_evidence":
      return {title:`עקוב אחרי: ${requirementLabel(action.requirementKey)}`, detail:"הראיה כבר התבקשה וטרם נאספה"};
    case "review_waiting_item":
      return {title:`בדוק סטטוס מול ${action.label??"הגורם החיצוני"}`, detail:`ממתינים ל${action.secondaryLabel??"פריט"}`};
    case "start_workstream":
      return {title:`התחל את מסלול ${workstreamLabel(action.workstreamKind)}`, detail:"המסלול רלוונטי לתיק אך טרם התחיל"};
    default:
      return {title:"קבע את הפעולה היזומה הבאה בתיק", detail:"לא נמצא כרגע אות תפעולי דחוף יותר"};
  }
}

function bandLabel(band:CaseHealth["band"]){
  if(band==="good")return "תקין תפעולית";
  if(band==="attention")return "דורש תשומת לב";
  return "סיכון תפעולי";
}

export function CaseHealthPanel({matterId}:{matterId:string}){
  const {data,loading,error}=useCommand(
    ()=>commands.get_case_health({matterId}) as Promise<CaseHealth>, [matterId]
  );
  if(loading)return <section className="workspace-card"><h2>בריאות תיק</h2><p className="quiet">מחשב מצב תפעולי...</p></section>;
  if(error||!data)return <section className="workspace-card"><h2>בריאות תיק</h2><p className="quiet">לא ניתן לחשב כרגע: {error??"שגיאה לא ידועה"}</p></section>;
  const next=actionText(data.nextBestAction);
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
      <div className="next-action"><strong>{next.title}</strong><p>{next.detail}</p></div>
      <p className="quiet">העדיפות נגזרת רק ממצב התיק הקיים; TAHRIR אינה מבצעת את הפעולה או מאשרת אותה בעצמה.</p>
    </section>
  </div>;
}
