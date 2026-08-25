import { StatusBadge } from "./StatusBadge";
import { useCommand } from "../lib/hooks";
import { commands } from "../lib/ipc";

type Health = {database:string; sourceIndex:string; ocrRuntime:string; aiProvider:string};

const TONE:Record<string,"ok"|"warn"|"risk"|"neutral"> = {
  ok:"ok", bound:"ok", enabled:"ok",
  not_configured:"warn", missing:"warn", disabled:"neutral",
  unreachable:"risk",
};
const LABEL:Record<string,string> = {
  ok:"תקין", bound:"מקומי", enabled:"פעיל",
  not_configured:"לא הוגדר", missing:"Runtime נדרש", disabled:"כבוי",
  unreachable:"לא זמין",
};

export function Inspector() {
  const {data:health}=useCommand(()=>commands.get_app_health({}) as Promise<Health>,[]);

  return <div className="inspector">
    <span className="eyebrow">INSPECTOR</span>
    <h3>מצב מערכת</h3>
    <div className="inspector-row"><span>מסד נתונים</span>
      <StatusBadge tone={health?TONE[health.database]??"neutral":"neutral"}>{health?LABEL[health.database]??health.database:"..."}</StatusBadge></div>
    <div className="inspector-row"><span>מקור קבצים</span>
      <StatusBadge tone={health?TONE[health.sourceIndex]??"neutral":"neutral"}>{health?LABEL[health.sourceIndex]??health.sourceIndex:"..."}</StatusBadge></div>
    <div className="inspector-row"><span>OCR</span>
      <StatusBadge tone={health?TONE[health.ocrRuntime]??"neutral":"neutral"}>{health?LABEL[health.ocrRuntime]??health.ocrRuntime:"..."}</StatusBadge></div>
    <div className="inspector-row"><span>AI</span>
      <StatusBadge tone={health?TONE[health.aiProvider]??"neutral":"neutral"}>{health?LABEL[health.aiProvider]??health.aiProvider:"..."}</StatusBadge></div>
    <p className="quiet">AI מציע בלבד. עובדות, מועדים, תחשיבים וטיוטות דורשים מסלול אישור מפורש.</p>
  </div>;
}
