import { useState } from "react";
import { matters } from "../lib/demo";
import { OverviewTab } from "../matter/OverviewTab";
import { DocumentsTab } from "../matter/DocumentsTab";
import { FactsAITab } from "../matter/FactsAITab";
import { DamageTab } from "../matter/DamageTab";
import { TasksCalendarTab } from "../matter/TasksCalendarTab";
import { LegalDocumentsTab } from "../matter/LegalDocumentsTab";
import { AuthoritiesTab } from "../matter/AuthoritiesTab";

type Tab="overview"|"documents"|"facts"|"damage"|"tasks"|"legal"|"research";

export function MatterWorkspace({matterId,onBack}:{matterId:string;onBack:()=>void}) {
  const matter=matters.find(m=>m.id===matterId) ?? matters[0]!;
  const [tab,setTab]=useState<Tab>("overview");
  const tabs:Array<[Tab,string]>=[["overview","סקירה"],["documents","מסמכים"],["facts","עובדות ו־AI"],["damage","נזק"],["tasks","משימות ויומן"],["legal","מסמכים משפטיים"],["research","מחקר"]];

  return <div className="page matter-page">
    <div className="matter-header">
      <div><button className="back-link" onClick={onBack}>→ תיקים</button><span className="eyebrow">{matter.internalNumber} · {matter.matterType}</span><h1>{matter.title}</h1><div className="meta-chips"><span>{matter.documentCount} מסמכים</span><span>{matter.verifiedFactCount} עובדות</span><span>{matter.workflowStage}</span></div></div>
      <div className="header-actions"><button className="btn secondary">פתח תיקייה</button><button className="btn primary">חלץ טקסט</button></div>
    </div>
    <nav className="matter-tabs" aria-label="אזורים בתיק">
      {tabs.map(([key,label])=><button key={key} aria-current={tab===key?"page":undefined} className={tab===key?"active":""} onClick={()=>setTab(key)}>{label}</button>)}
    </nav>
    <div className="tab-body">
      {tab==="overview"?<OverviewTab matter={matter}/>:tab==="documents"?<DocumentsTab/>:tab==="facts"?<FactsAITab/>:tab==="damage"?<DamageTab/>:tab==="tasks"?<TasksCalendarTab/>:tab==="legal"?<LegalDocumentsTab/>:<AuthoritiesTab/>}
    </div>
  </div>;
}
