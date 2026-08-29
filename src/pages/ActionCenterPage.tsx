import { useState } from "react";
import { loadActionCenter } from "../lib/actionCenter";
import { useCommand } from "../lib/hooks";
import { ActionCandidateRow } from "../components/ActionCandidateRow";

const FILTERS = [["all", "הכל"], ["overdue", "באיחור"], ["blocking", "חוסם התקדמות"], ["normal", "רגיל"]] as const;

export function ActionCenterPage({ onOpenMatter }: { onOpenMatter: (matterId: string) => void }) {
  const { data: entries, loading, error, reload } = useCommand(loadActionCenter, []);
  const [filter, setFilter] = useState<typeof FILTERS[number][0]>("all");

  const rows = (entries ?? []).flatMap(e =>
    e.plan.candidates
      .filter(c => filter === "all" || c.urgency === filter)
      .map(c => ({ candidate: c, matterTitle: e.matterTitle })),
  );

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">OPERATIONS</span><h1>מרכז פעולה</h1>
      <p>ה־backlog התפעולי המלא של כל התיקים הפעילים, מדורג לפי אותו מנוע דירוג בלבד - ללא מיון נוסף בצד הלקוח.</p></div></div>
    <div className="filterbar">{FILTERS.map(([key, label]) =>
      <button key={key} className={filter === key ? "active" : ""} onClick={() => setFilter(key)}>{label}</button>,
    )}</div>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && rows.length === 0 && <p className="quiet">אין פריטים.</p>}
    <div className="dense-list">
      {rows.map(({ candidate, matterTitle }) => (
        <ActionCandidateRow
          key={candidate.fingerprint}
          candidate={candidate}
          matterTitle={matterTitle}
          onOpenMatter={onOpenMatter}
          onChanged={reload}
        />
      ))}
    </div>
  </div>;
}
