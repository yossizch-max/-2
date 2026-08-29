import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { SegmentedSubTabs } from "../components/SegmentedSubTabs";
import type { Workstream } from "../types";
import { MedicalEvidenceTab } from "./MedicalEvidenceTab";
import { MedicalTimelineTab } from "./MedicalTimelineTab";
import { MedicalBriefTab } from "./MedicalBriefTab";
import { WageEvidenceTab } from "./WageEvidenceTab";
import { WageTimelineTab } from "./WageTimelineTab";
import { WageBriefTab } from "./WageBriefTab";
import { LiabilityEvidenceTab } from "./LiabilityEvidenceTab";
import { LiabilityBriefTab } from "./LiabilityBriefTab";
import { DamageTab } from "./DamageTab";
import { NegotiationTab } from "./NegotiationTab";
import { WorkstreamsTab } from "./WorkstreamsTab";
import { MissingEvidenceTab } from "./MissingEvidenceTab";
import { LedgersTab } from "./LedgersTab";
import { TasksCalendarTab } from "./TasksCalendarTab";

type CardKey="medical"|"wage"|"liability"|"damageNegotiation"|"management";

const CARDS:Array<[CardKey,string,string]>=[
  ["medical","🩺","רפואי"],
  ["wage","💰","שכר והכנסה"],
  ["liability","⚖️","אחריות"],
  ["damageNegotiation","🤝","נזק ומו״מ"],
  ["management","🗂️","ניהול תיק"],
];

// UX Milestone 1: "עבודת התיק" - a workstream/ledger/missing-evidence concept never
// appears as its own top-level tab or as raw backend language here; each card shows
// only a colored dot + one word, and the underlying kind (workstreams.rs's own
// ALLOWED_KINDS) is never printed on screen.
const WORKSTREAM_KIND_FOR_CARD:Record<CardKey,string|null>={
  medical:"medical", wage:"wage", liability:"liability", damageNegotiation:"negotiation", management:null,
};
function statusDotClass(status?:string){
  if(status==="blocked")return "blocked";
  if(status==="active")return "active-status";
  if(status==="not_started")return "not-started";
  return "";
}
function statusLabel(status?:string){
  if(status==="blocked")return "חסום";
  if(status==="active")return "פעיל";
  if(status==="done")return "הושלם";
  if(status==="not_started")return "טרם התחיל";
  return "";
}

export function MatterWorkTab({matterId}:{matterId:string}) {
  const [open,setOpen]=useState<CardKey|null>(null);
  const {data:workstreams}=useCommand(
    ()=>commands.list_matter_workstreams({matterId}) as Promise<Workstream[]>, [matterId]
  );
  const statusFor=(card:CardKey)=>{
    const kind=WORKSTREAM_KIND_FOR_CARD[card];
    if(!kind)return undefined;
    return workstreams?.find(w=>w.kind===kind)?.status;
  };

  if(open){
    const content=
      open==="medical" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<MedicalEvidenceTab matterId={matterId}/>],
          ["timeline","ציר זמן",<MedicalTimelineTab matterId={matterId}/>],
          ["brief","תדריך",<MedicalBriefTab matterId={matterId}/>],
        ]}/>
      : open==="wage" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<WageEvidenceTab matterId={matterId}/>],
          ["timeline","ציר זמן",<WageTimelineTab matterId={matterId}/>],
          ["brief","תדריך",<WageBriefTab matterId={matterId}/>],
        ]}/>
      : open==="liability" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<LiabilityEvidenceTab matterId={matterId}/>],
          ["brief","תדריך",<LiabilityBriefTab matterId={matterId}/>],
        ]}/>
      : open==="damageNegotiation" ? <SegmentedSubTabs segments={[
          ["damage","נזק",<DamageTab matterId={matterId}/>],
          ["negotiation","מו״מ וביטוח",<NegotiationTab matterId={matterId}/>],
        ]}/>
      : <SegmentedSubTabs segments={[
          ["workstreams","מסלולי עבודה",<WorkstreamsTab matterId={matterId}/>],
          ["evidence","ראיות חסרות",<MissingEvidenceTab matterId={matterId}/>],
          ["ledgers","פנקסים",<LedgersTab matterId={matterId}/>],
          ["tasks","משימות ויומן",<TasksCalendarTab matterId={matterId}/>],
        ]}/>;
    return <div>
      <button className="btn secondary" onClick={()=>setOpen(null)} style={{marginBottom:14}}>← חזרה לעבודת התיק</button>
      {content}
    </div>;
  }

  return <div className="work-cards">
    {CARDS.map(([key,icon,label])=>{
      const status=statusFor(key);
      return <button key={key} className="work-card" onClick={()=>setOpen(key)}>
        <span style={{fontSize:22}}>{icon}</span>
        <strong>{label}</strong>
        {status && <small className="quiet">
          <span className={`status-dot ${statusDotClass(status)}`}/>{statusLabel(status)}
        </small>}
      </button>;
    })}
  </div>;
}
