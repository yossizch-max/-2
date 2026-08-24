import { StatusBadge } from "./StatusBadge";
export function Inspector() {
  return <div className="inspector">
    <span className="eyebrow">INSPECTOR</span>
    <h3>מצב מערכת</h3>
    <div className="inspector-row"><span>מסד נתונים</span><StatusBadge tone="ok">SQLCipher</StatusBadge></div>
    <div className="inspector-row"><span>מקור קבצים</span><StatusBadge tone="ok">מקומי</StatusBadge></div>
    <div className="inspector-row"><span>OCR</span><StatusBadge tone="warn">Runtime נדרש</StatusBadge></div>
    <div className="inspector-row"><span>AI</span><StatusBadge>כבוי</StatusBadge></div>
    <p className="quiet">AI מציע בלבד. עובדות, מועדים, תחשיבים וטיוטות דורשים מסלול אישור מפורש.</p>
  </div>;
}
