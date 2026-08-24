import { useState } from "react";
import type { ChangeEvent } from "react";
import { documents, matters } from "../lib/demo";
export function SearchPage() {
  const [q,setQ]=useState("");
  const rows=[
    ...matters.map(m=>({type:"תיק",title:m.title,sub:m.internalNumber||""})),
    ...documents.map(d=>({type:"מסמך",title:d.fileName,sub:d.category}))
  ].filter(x=>(x.title+" "+x.sub).includes(q));
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">SEARCH</span><h1>חיפוש</h1><p>Metadata, שמות קבצים, טקסט מחולץ ועובדות מאומתות.</p></div></div>
    <input className="big-search" value={q} onChange={(e: ChangeEvent<HTMLInputElement>)=>setQ(e.target.value)} placeholder="שם תיק, מספר או מסמך..."/>
    <div className="dense-list">{rows.map((x,i)=><button className="work-row" key={i}><div><span className="eyebrow">{x.type}</span><strong>{x.title}</strong><small>{x.sub}</small></div><span>פתח</span></button>)}</div>
  </div>;
}
