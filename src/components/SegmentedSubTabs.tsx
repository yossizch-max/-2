import { useState, type ReactNode } from "react";

// UX Milestone 1: a lightweight two-level nav primitive. Several previously
// top-level tabs (evidence/timeline/brief per domain, understanding/timeline/brief
// at the matter level, legal documents/research) are grouped under one of the four
// primary matter entries (בית התיק / מסמכים / עבודת התיק / ניסוח) - this renders
// that inner switch without introducing any new routing concept or backend call.
export function SegmentedSubTabs({
  segments, initial,
}: {
  segments: Array<[string, string, ReactNode]>; // [key, label, content]
  initial?: string;
}) {
  const [active, setActive] = useState(initial ?? segments[0]?.[0] ?? "");
  const current = segments.find(([key]) => key === active) ?? segments[0];
  return <div className="segmented-sub">
    <div className="segmented-sub-row" role="tablist">
      {segments.map(([key, label]) => (
        <button key={key} role="tab" aria-selected={active === key}
          className={active === key ? "active" : ""} onClick={() => setActive(key)}>
          {label}
        </button>
      ))}
    </div>
    <div className="segmented-sub-body">{current?.[2]}</div>
  </div>;
}
