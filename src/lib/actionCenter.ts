import { commands } from "./ipc";
import type { ActionItem, Matter, Task, Deadline } from "../types";

type WaitingFor={id:string;matterId:string;partyLabel:string;itemLabel:string;followUpAt?:string|null;status:string};

export async function loadActionItems():Promise<ActionItem[]>{
  const matters=(await commands.list_matters() as Matter[]).filter(m=>m.status==="active");
  const items:ActionItem[]=[];

  await Promise.all(matters.map(async matter=>{
    const [deadlines,tasks,waiting]=await Promise.all([
      commands.list_deadlines({matterId:matter.id}) as Promise<Deadline[]>,
      commands.list_tasks({matterId:matter.id}) as Promise<Task[]>,
      commands.list_waiting_for({matterId:matter.id}) as Promise<WaitingFor[]>,
    ]);

    for(const d of deadlines){
      if(d.state!=="committed")continue;
      items.push({id:`dl-${d.id}`,matterId:matter.id,matterTitle:matter.title,kind:"critical",
        title:`מועד: ${d.action} · ${d.dueAt}`,subtitle:d.sourceLabel,actionLabel:"פתח מועד",dueAt:d.dueAt});
    }
    for(const t of tasks){
      if(t.status!=="open")continue;
      items.push({id:`t-${t.id}`,matterId:matter.id,matterTitle:matter.title,
        kind:t.riskClass==="approval_required"?"review":"new",
        title:t.title,subtitle:t.dueAt?`יעד: ${t.dueAt}`:"אין יעד",actionLabel:"בדוק",dueAt:t.dueAt});
    }
    for(const w of waiting){
      if(w.status!=="open")continue;
      items.push({id:`w-${w.id}`,matterId:matter.id,matterTitle:matter.title,kind:"waiting",
        title:`ממתין ל${w.partyLabel}: ${w.itemLabel}`,
        subtitle:w.followUpAt?`Follow-up: ${w.followUpAt}`:"אין תאריך מעקב",actionLabel:"פתח"});
    }
  }));

  const order:Record<ActionItem["kind"],number>={critical:0,review:1,waiting:2,resume:3,new:4};
  return items.sort((a,b)=>{
    if(a.dueAt&&b.dueAt&&a.dueAt!==b.dueAt)return a.dueAt<b.dueAt?-1:1;
    if(a.dueAt&&!b.dueAt)return -1;
    if(!a.dueAt&&b.dueAt)return 1;
    return order[a.kind]-order[b.kind];
  });
}
