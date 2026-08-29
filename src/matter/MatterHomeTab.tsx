import { SegmentedSubTabs } from "../components/SegmentedSubTabs";
import { OverviewTab } from "./OverviewTab";
import { UnderstandingTab } from "./UnderstandingTab";
import { MatterTimelineTab } from "./MatterTimelineTab";
import { MatterBriefTab } from "./MatterBriefTab";
import { RequirementsPanel } from "./MissingEvidenceTab";
import type { Matter } from "../types";

// UX Milestone 1: "בית התיק" - the always-first entry into a matter. Direct
// document intake (OverviewTab) is the default view; matter-level understanding,
// timeline and brief are one click away as segments of the same screen rather than
// three more top-level tabs to remember. The AI findings queue is a Milestone 3
// experience and is intentionally not present here yet. Missing evidence has no
// dedicated tab either - it appears directly below as a persistent, case-wide
// panel, always visible regardless of which segment above is active.
export function MatterHomeTab({matter,onMatterChanged}:{matter:Matter;onMatterChanged:()=>void}) {
  return <div>
    <SegmentedSubTabs
      initial="summary"
      segments={[
        ["summary","סיכום",<OverviewTab matter={matter} onMatterChanged={onMatterChanged}/>],
        ["understanding","הבנת התיק",<UnderstandingTab matterId={matter.id}/>],
        ["timeline","ציר זמן",<MatterTimelineTab matterId={matter.id}/>],
        ["brief","תדריך תיק",<MatterBriefTab matterId={matter.id}/>],
      ]}
    />
    <RequirementsPanel matterId={matter.id}/>
  </div>;
}
