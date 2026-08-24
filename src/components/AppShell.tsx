import type { ReactNode } from "react";
export type NavKey = "today"|"matters"|"actions"|"calendar"|"search"|"templates"|"ai"|"settings";

const nav: Array<[NavKey,string,string]> = [
  ["today","היום","⌂"],["matters","תיקים","▣"],["actions","מרכז פעולה","✓"],["calendar","יומן","□"],
  ["search","חיפוש","⌕"],["templates","תבניות","▤"],["ai","AI","AI"],["settings","הגדרות","⚙"]
];

export function AppShell({active,onNavigate,onCommand,children,inspector}:{
  active:NavKey; onNavigate:(k:NavKey)=>void; onCommand:()=>void; children:ReactNode; inspector:ReactNode
}) {
  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><b>ת</b><div><strong>TAHRIR</strong><small>Legal Workspace</small></div></div>
      <button className="global-search" onClick={onCommand}><span>חיפוש או פעולה</span><kbd>Ctrl K</kbd></button>
      <div className="health-inline"><span className="health-dot"/><span>מקומי · מאובטח</span></div>
    </header>
    <aside className="inspector-pane">{inspector}</aside>
    <main className="workspace">{children}</main>
    <nav className="nav-rail" aria-label="ניווט ראשי">
      <div className="nav-main">
        {nav.map(([key,label,icon]) => <button key={key} aria-current={active===key?"page":undefined} className={active===key?"active":""} onClick={()=>onNavigate(key)}>
          <span className="nav-icon">{icon}</span><span>{label}</span>
        </button>)}
      </div>
    </nav>
  </div>;
}
