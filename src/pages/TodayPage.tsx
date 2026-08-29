import { loadActionCenter } from "../lib/actionCenter";
import { useCommand } from "../lib/hooks";
import { ActionCandidateRow } from "../components/ActionCandidateRow";
import type { ActionCandidate } from "../types";

function dayKey(d: Date) { return d.toISOString().slice(0, 10); }

// Pure display grouping over the backend's own ranked list - it changes
// where a candidate is *shown*, never its rank or whether it is the primary
// action. Candidates keep the backend's own order inside each bucket.
function bucketOf(item: ActionCandidate, todayKey: string, tomorrowKey: string, weekAheadKey: string): string {
  if (item.dueAt) {
    if (item.dueAt < todayKey) return "overdue";
    if (item.dueAt === todayKey) return "today";
    if (item.dueAt === tomorrowKey) return "tomorrow";
    if (item.dueAt <= weekAheadKey) return "week";
  }
  if (item.urgency === "blocking") return "blocking";
  if (item.rankCategory <= 8) return "review";
  return "resume";
}

const BUCKET_LABELS: Record<string, string> = {
  overdue: "באיחור", today: "היום", tomorrow: "מחר", week: "7 הימים הקרובים",
  blocking: "חוסם התקדמות", review: "דורש החלטת עו\"ד", resume: "להמשך עבודה",
};
const BUCKET_ORDER = ["overdue", "today", "tomorrow", "week", "blocking", "review", "resume"];

export function TodayPage({ onOpenMatter }: { onOpenMatter: (matterId: string) => void }) {
  const { data: entries, loading, error, reload } = useCommand(loadActionCenter, []);
  const candidates = (entries ?? []).flatMap(e =>
    e.plan.candidates.map(c => ({ candidate: c, matterTitle: e.matterTitle })),
  );

  const now = new Date();
  const todayKey = dayKey(now);
  const tomorrowKey = dayKey(new Date(now.getTime() + 86400000));
  const weekAheadKey = dayKey(new Date(now.getTime() + 7 * 86400000));

  const buckets: Record<string, typeof candidates> = {};
  for (const row of candidates) {
    const b = bucketOf(row.candidate, todayKey, tomorrowKey, weekAheadKey);
    (buckets[b] ??= []).push(row);
  }
  const overdueCount = buckets.overdue?.length ?? 0;
  const todayCount = buckets.today?.length ?? 0;
  const weekCount = buckets.week?.length ?? 0;
  const reviewCount = buckets.review?.length ?? 0;

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">יום עבודה</span><h1>היום</h1><p>מה יכול לפגוע היום, מה דורש אישור ומה ממתין - מאותו מנוע דירוג של מרכז הפעולה.</p></div></div>
    {!loading && !error && candidates.length > 0 && <p className="quiet">
      {overdueCount > 0 && <>{overdueCount} באיחור · </>}
      {todayCount} להיום · {weekCount} ב-7 הימים הקרובים · {reviewCount} דורשים אישור
    </p>}
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && candidates.length === 0 && <p className="quiet">אין פריטים פתוחים כרגע.</p>}
    {BUCKET_ORDER.map(bucket => {
      const rows = buckets[bucket] ?? [];
      if (!rows.length) return null;
      return <section className="work-section" key={bucket}>
        <div className="section-head"><h2>{BUCKET_LABELS[bucket]}</h2><span>{rows.length}</span></div>
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
      </section>;
    })}
  </div>;
}
