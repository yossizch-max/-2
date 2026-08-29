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
import { MedicalSection, WageSection, LiabilitySection } from "./LedgersTab";
import { RequirementsPanel } from "./MissingEvidenceTab";

type CardKey="medical"|"wage"|"liability"|"damageNegotiation";

const CARDS:Array<[CardKey,string,string]>=[
  ["medical","🩺","רפואי"],
  ["wage","💰","שכר והכנסה"],
  ["liability","⚖️","אחריות"],
  ["damageNegotiation","🤝","נזק ומו״מ"],
];

// UX Milestone 1: "עבודת התיק" has exactly these four domain cards - no fifth
// "ניהול תיק" card, and no Workstreams/Ledgers/Missing Evidence/Tasks sub-tabs.
// Their backend functionality is preserved without a dedicated tab: a workstream's
// status only ever drives the card's own status dot below (never an editable list
// here); ledger data (MedicalSection/WageSection/LiabilitySection) and missing-
// evidence items (RequirementsPanel, filtered to the domain) render directly inside
// the relevant domain's own evidence/negotiation view; tasks/calendar stay reachable
// through the global "משימות ויומן" nav destination, not from inside a matter.
const WORKSTREAM_KIND_FOR_CARD:Record<CardKey,string>={
  medical:"medical", wage:"wage", liability:"liability", damageNegotiation:"negotiation",
};

const MISSING_EVIDENCE_KEYS_FOR_CARD:Record<CardKey,string[]>={
  medical:["medical_records_initial","medical_records_full_file","expert_opinion"],
  wage:["wage_stubs","employer_incident_report"],
  liability:["police_report","witness_statements","vehicle_photos"],
  damageNegotiation:["insurance_policy"],
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
  const statusFor=(card:CardKey)=>workstreams?.find(w=>w.kind===WORKSTREAM_KIND_FOR_CARD[card])?.status;

  if(open){
    const content=
      open==="medical" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<>
            <MedicalSection matterId={matterId}/>
            <RequirementsPanel matterId={matterId} filterKeys={MISSING_EVIDENCE_KEYS_FOR_CARD.medical} title="ראיות חסרות - רפואי"/>
            <MedicalEvidenceTab matterId={matterId}/>
          </>],
          ["timeline","ציר זמן",<MedicalTimelineTab matterId={matterId}/>],
          ["brief","תדריך",<MedicalBriefTab matterId={matterId}/>],
        ]}/>
      : open==="wage" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<>
            <WageSection matterId={matterId}/>
            <RequirementsPanel matterId={matterId} filterKeys={MISSING_EVIDENCE_KEYS_FOR_CARD.wage} title="ראיות חסרות - שכר והכנסה"/>
            <WageEvidenceTab matterId={matterId}/>
          </>],
          ["timeline","ציר זמן",<WageTimelineTab matterId={matterId}/>],
          ["brief","תדריך",<WageBriefTab matterId={matterId}/>],
        ]}/>
      : open==="liability" ? <SegmentedSubTabs segments={[
          ["evidence","ראיות",<>
            <LiabilitySection matterId={matterId}/>
            <RequirementsPanel matterId={matterId} filterKeys={MISSING_EVIDENCE_KEYS_FOR_CARD.liability} title="ראיות חסרות - אחריות"/>
            <LiabilityEvidenceTab matterId={matterId}/>
          </>],
          ["brief","תדריך",<LiabilityBriefTab matterId={matterId}/>],
        ]}/>
      : <SegmentedSubTabs segments={[
          ["damage","נזק",<DamageTab matterId={matterId}/>],
          ["negotiation","מו״מ וביטוח",<>
            <RequirementsPanel matterId={matterId} filterKeys={MISSING_EVIDENCE_KEYS_FOR_CARD.damageNegotiation} title="ראיות חסרות - נזק ומו״מ"/>
            <NegotiationTab matterId={matterId}/>
          </>],
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
