import type { ReactNode } from "react";
export type NavKey = "today"|"matters"|"actions"|"calendar"|"search"|"templates"|"ai"|"settings";

// UX Milestone 1: the primary rail carries only the four destinations a lawyer
// reaches for constantly (Today / Matters / Tasks & Calendar / Search). Action
// Center, Templates and AI are capabilities, not destinations - they stay fully
// reachable (Ctrl+K, or from Settings) without competing for rail space; Settings
// itself is a secondary, visually distinct entry rather than a sixth peer button.
const primaryNav: Array<[NavKey,string,string]> = [
  ["today","היום","⌂"],["matters","תיקים","▣"],["calendar","משימות ויומן","□"],["search","חיפוש","⌕"],
];

export function AppShell({active,onNavigate,onCommand,children}:{
  active:NavKey; onNavigate:(k:NavKey)=>void; onCommand:()=>void; children:ReactNode
}) {
  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><b>ת</b><div><strong>TAHRIR</strong><small>Legal Workspace</small></div></div>
      <button className="global-search" onClick={onCommand}><span>חיפוש או פעולה</span><kbd>Ctrl K</kbd></button>
      <div className="health-inline"><span className="health-dot"/><span>מקומי · מאובטח</span></div>
    </header>
    <main className="workspace">{children}</main>
    <nav className="nav-rail" aria-label="ניווט ראשי">
      <div className="nav-main">
        {primaryNav.map(([key,label,icon]) => <button key={key} aria-current={active===key?"page":undefined} className={active===key?"active":""} onClick={()=>onNavigate(key)}>
          <span className="nav-icon">{icon}</span><span>{label}</span>
        </button>)}
      </div>
      <div className="nav-secondary">
        <button aria-current={active==="settings"?"page":undefined} className={active==="settings"?"active":""} onClick={()=>onNavigate("settings")}>
          <span className="nav-icon">⚙</span><span>הגדרות</span>
        </button>
      </div>
    </nav>
  </div>;
}
