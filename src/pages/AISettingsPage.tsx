import { StatusBadge } from "../components/StatusBadge";
export function AISettingsPage() {
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">AI CONTROL</span><h1>AI</h1><p>חיבור טכני ואישור נתוני לקוח הם שני שערים נפרדים.</p></div></div>
    <div className="grid-2">
      <section className="workspace-card"><div className="card-head"><h2>Local compatible</h2><StatusBadge tone="ok">Loopback only</StatusBadge></div><label>Endpoint<input value="http://127.0.0.1:11434/v1" readOnly/></label><button className="btn secondary">בדיקה סינתטית</button></section>
      <section className="workspace-card"><div className="card-head"><h2>OpenAI</h2><StatusBadge>לא מאושר לחומר לקוח</StatusBadge></div><p>Endpoint קבוע. Redirects כבויים. store:false. הסוד נשמר ב־OS Credential Store.</p><button className="btn secondary">הגדר מפתח</button></section>
    </div>
    <section className="workspace-card"><h2>Client Data Gate</h2><p className="warning-box">חיבור תקין לספק אינו מרשה שליחת חומר מתיק. כל egress חיצוני דורש הרשאה מפורשת.</p></section>
  </div>;
}
