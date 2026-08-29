import { SegmentedSubTabs } from "../components/SegmentedSubTabs";
import { LegalDocumentsTab } from "./LegalDocumentsTab";
import { AuthoritiesTab } from "./AuthoritiesTab";

// UX Milestone 1: "ניסוח" - legal drafting and the research/authorities that back
// it belong together for a lawyer, so they share one top-level entry instead of
// two unrelated tabs ("מסמכים משפטיים" / "מחקר").
export function MatterDraftingTab({matterId}:{matterId:string}) {
  return <SegmentedSubTabs
    segments={[
      ["documents","מסמכים משפטיים",<LegalDocumentsTab matterId={matterId}/>],
      ["research","אסמכתאות",<AuthoritiesTab matterId={matterId}/>],
    ]}
  />;
}
