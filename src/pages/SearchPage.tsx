import { useEffect, useState } from "react";
import type { ChangeEvent } from "react";
import { commands } from "../lib/ipc";

type SearchHit = {kind:string; matterId?:string|null; id:string; title:string; subtitle:string};
const KIND_LABEL:Record<string,string>={matter:"תיק",file:"מסמך",verified_fact:"עובדה"};

export function SearchPage() {
  const [q,setQ]=useState("");
  const [rows,setRows]=useState<SearchHit[]>([]);
  const [loading,setLoading]=useState(false);

  useEffect(()=>{
    if(q.trim().length<2){setRows([]);return;}
    let cancelled=false;
    setLoading(true);
    const t=setTimeout(()=>{
      (commands.search_everything({query:q}) as Promise<SearchHit[]>)
        .then(res=>{if(!cancelled){setRows(res);setLoading(false);}})
        .catch(()=>{if(!cancelled)setLoading(false);});
    },250);
    return()=>{cancelled=true;clearTimeout(t);};
  },[q]);

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">SEARCH</span><h1>חיפוש</h1><p>Metadata, שמות קבצים, טקסט מחולץ ועובדות מאומתות.</p></div></div>
    <input className="big-search" value={q} onChange={(e: ChangeEvent<HTMLInputElement>)=>setQ(e.target.value)} placeholder="שם תיק, מספר או מסמך..."/>
    {loading && <p className="quiet">מחפש...</p>}
    <div className="dense-list">{rows.map((x,i)=><button className="work-row" key={i}><div><span className="eyebrow">{KIND_LABEL[x.kind]??x.kind}</span><strong>{x.title}</strong><small>{x.subtitle}</small></div><span>פתח</span></button>)}</div>
    {!loading && q.trim().length>=2 && rows.length===0 && <p className="quiet">אין תוצאות.</p>}
  </div>;
}
