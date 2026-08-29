import { useState } from "react";
import { commands } from "../lib/ipc";
import { setRecommendationState, convertActionToTask, markDeadlineSatisfied } from "../lib/actionCenter";
import { StatusBadge } from "./StatusBadge";
import type { ActionCandidate } from "../types";

const URGENCY_TONE: Record<string, "ok" | "warn" | "risk" | "neutral"> = {
  overdue: "risk", due_soon: "warn", blocking: "warn", attention: "warn", normal: "neutral", backlog: "neutral",
};
const URGENCY_LABELS: Record<string, string> = {
  overdue: "באיחור", due_soon: "מתקרב", blocking: "חוסם התקדמות", attention: "דורש תשומת לב",
  normal: "רגיל", backlog: "המשך עבודה",
};

/// A row for one deterministic Action Candidate - used by TodayPage,
/// ActionCenterPage, and CaseHealthPanel so every surface shows the same
/// candidate the same way, with the same controls. Nothing here changes the
/// candidate's rank or urgency; every button is an explicit human action that
/// calls one backend command and nothing else.
export function ActionCandidateRow({
  candidate, matterTitle, onOpenMatter, onChanged,
}: {
  candidate: ActionCandidate;
  matterTitle?: string;
  onOpenMatter?: (matterId: string) => void;
  onChanged?: () => void;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [showSnooze, setShowSnooze] = useState(false);
  const [snoozeUntil, setSnoozeUntil] = useState("");

  const run = async (key: string, action: () => Promise<unknown>) => {
    setBusy(key);
    setErr(null);
    try {
      await action();
      onChanged?.();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const options = candidate.humanActionOptions;
  const dismissed = candidate.recommendationState === "dismissed";
  const snoozed = candidate.recommendationState === "snoozed";

  return <div className={`proposal action-candidate-row${dismissed ? " quiet" : ""}`}>
    <div className="header-actions">
      <strong>{candidate.title}</strong>
      <StatusBadge tone={URGENCY_TONE[candidate.urgency] ?? "neutral"}>
        {URGENCY_LABELS[candidate.urgency] ?? candidate.urgency}
      </StatusBadge>
    </div>
    {matterTitle && <small className="quiet">{matterTitle}</small>}
    <p>{candidate.reason}</p>
    <small className="quiet">
      מקור / טריגר: {candidate.sourceType}{candidate.dueAt ? ` · יעד: ${candidate.dueAt}` : ""}
    </small>
    {snoozed && candidate.snoozedUntil && <p className="quiet">הושהה עד {candidate.snoozedUntil}</p>}
    {dismissed && <p className="quiet">נדחה על ידך</p>}
    {err && <p className="quiet">{err}</p>}

    {showSnooze && <div className="header-actions" style={{ marginTop: 6 }}>
      <input type="date" value={snoozeUntil} onChange={e => setSnoozeUntil(e.target.value)} />
      <button className="btn secondary" disabled={!snoozeUntil || busy !== null}
        onClick={() => run("snooze", () => setRecommendationState(candidate.matterId, candidate.fingerprint, "snoozed", snoozeUntil))}>
        אשר השהיה
      </button>
      <button className="btn secondary" onClick={() => setShowSnooze(false)}>ביטול</button>
    </div>}

    <div className="header-actions" style={{ marginTop: 6 }}>
      {onOpenMatter && <button className="btn secondary" onClick={() => onOpenMatter(candidate.matterId)}>פתח בתיק</button>}

      {options.includes("mark_satisfied") && candidate.targetId && <button className="btn primary" disabled={busy !== null}
        onClick={() => run("satisfy", () => markDeadlineSatisfied(candidate.targetId as string))}>
        סמן כטופל
      </button>}

      {options.includes("complete") && candidate.targetId && <button className="btn primary" disabled={busy !== null}
        onClick={() => run("complete", () => commands.complete_task({ taskId: candidate.targetId }))}>
        סמן כבוצע
      </button>}

      {options.includes("close") && candidate.targetId && <button className="btn secondary" disabled={busy !== null}
        onClick={() => run("close", () => commands.close_waiting_for({ matterId: candidate.matterId, waitingForId: candidate.targetId }))}>
        סגור מעקב
      </button>}

      {options.includes("create_task") && !dismissed && candidate.recommendationState !== "converted_to_task" && (
        <button className="btn secondary" disabled={busy !== null}
          onClick={() => run("task", () => convertActionToTask(candidate.matterId, candidate.fingerprint))}>
          צור משימה
        </button>
      )}

      {(options.includes("snooze") || options.includes("snooze_display")) && !showSnooze && (
        <button className="btn secondary" disabled={busy !== null} onClick={() => setShowSnooze(true)}>השהה</button>
      )}

      {options.includes("dismiss") && !dismissed && (
        <button className="btn secondary" disabled={busy !== null}
          onClick={() => run("dismiss", () => setRecommendationState(candidate.matterId, candidate.fingerprint, "dismissed"))}>
          התעלם
        </button>
      )}
    </div>
  </div>;
}
