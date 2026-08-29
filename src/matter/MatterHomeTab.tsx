import { SegmentedSubTabs } from "../components/SegmentedSubTabs";
import { OverviewTab } from "./OverviewTab";
import { UnderstandingTab } from "./UnderstandingTab";
import { MatterTimelineTab } from "./MatterTimelineTab";
import { MatterBriefTab } from "./MatterBriefTab";
import { FactsAITab } from "./FactsAITab";
import type { Matter } from "../types";

// UX Milestone 1: "בית התיק" - the always-first entry into a matter. Direct
// document intake (OverviewTab) is the default view; matter-level understanding,
// timeline, brief, and the AI findings queue are one click away as segments of the
// same screen rather than four more top-level tabs to remember.
export function MatterHomeTab({matter,onMatterChanged}:{matter:Matter;onMatterChanged:()=>void}) {
  return <SegmentedSubTabs
    initial="summary"
    segments={[
      ["summary","סיכום",<OverviewTab matter={matter} onMatterChanged={onMatterChanged}/>],
      ["understanding","הבנת התיק",<UnderstandingTab matterId={matter.id}/>],
      ["timeline","ציר זמן",<MatterTimelineTab matterId={matter.id}/>],
      ["brief","תדריך תיק",<MatterBriefTab matterId={matter.id}/>],
      ["ai","הצעות AI",<FactsAITab matterId={matter.id}/>],
    ]}
  />;
}
