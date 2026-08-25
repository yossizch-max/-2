import { loadActionItems } from "../lib/actionCenter";
import { useCommand } from "../lib/hooks";
import type { ActionItem } from "../types";

function dayKey(d:Date){ return d.toISOString().slice(0,10); }

function bucketOf(item:ActionItem, todayKey:string, tomorrowKey:string, weekAheadKey:string):string{
  if(item.dueAt){
    if(item.dueAt<todayKey)return "overdue";
    if(item.dueAt===todayKey)return "today";
    if(item.dueAt===tomorrowKey)return "tomorrow";
    if(item.dueAt<=weekAheadKey)return "week";
  }
  if(item.kind==="review")return "review";
  if(item.kind==="waiting")return "waiting";
  return "resume";
}

const BUCKET_LABELS:Record<string,string>={
  overdue:"באיחור", today:"היום", tomorrow:"מחר", week:"7 הימים הקרובים",
  review:"דורש החלטת עו\"ד", waiting:"ממתין לתגובה", resume:"להמשך עבודה",
};
const BUCKET_ORDER=["overdue","today","tomorrow","week","review","waiting","resume"];

export function TodayPage({onOpenMatter}:{onOpenMatter:(matterId:string)=>void}) {
  const {data:today,loading,error}=useCommand(loadActionItems,[]);

  const now=new Date();
  const todayKey=dayKey(now);
  const tomorrowKey=dayKey(new Date(now.getTime()+86400000));
  const weekAheadKey=dayKey(new Date(now.getTime()+7*86400000));

  const buckets:Record<string,ActionItem[]>={};
  for(const item of today??[]){
    const b=bucketOf(item,todayKey,tomorrowKey,weekAheadKey);
    (buckets[b]??=[]).push(item);
  }
  const overdueCount=buckets.overdue?.length??0;
  const todayCount=buckets.today?.length??0;
  const weekCount=buckets.week?.length??0;
  const reviewCount=buckets.review?.length??0;

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">יום עבודה</span><h1>היום</h1><p>מה יכול לפגוע היום, מה דורש אישור ומה ממתין.</p></div></div>
    {!loading && !error && (today?.length??0)>0 && <p className="quiet">
      {overdueCount>0 && <>{overdueCount} באיחור · </>}
      {todayCount} להיום · {weekCount} ב-7 הימים הקרובים · {reviewCount} דורשים אישור
    </p>}
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && today?.length===0 && <p className="quiet">אין פריטים פתוחים כרגע.</p>}
    {BUCKET_ORDER.map(bucket=>{
      const rows=buckets[bucket]??[];
      if(!rows.length) return null;
      return <section className="work-section" key={bucket}>
        <div className="section-head"><h2>{BUCKET_LABELS[bucket]}</h2><span>{rows.length}</span></div>
        <div className="dense-list">{rows.map(x=><button className="work-row" key={x.id}
          onClick={()=>x.matterId&&onOpenMatter(x.matterId)}>
          <div><strong>{x.title}</strong><small>{x.matterTitle} · {x.subtitle}</small></div><span>{x.actionLabel}</span>
        </button>)}</div>
      </section>;
    })}
  </div>;
}
