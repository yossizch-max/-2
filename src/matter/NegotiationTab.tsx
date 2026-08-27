import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";

type InsuranceClaim = {
  id:string; matterId:string; insurerName:string; claimNumber?:string|null; policyNumber?:string|null;
  handlerName?:string|null; handlerContact?:string|null;
  status:"open"|"awaiting_response"|"negotiating"|"settled"|"closed";
  notes?:string|null; createdAt:string; updatedAt:string;
};
type NegotiationPosition = {
  id:string; matterId:string; insuranceClaimId?:string|null; side:"our_side"|"counterparty";
  kind:"demand"|"offer"|"counter_offer"; amountCents:number; currency:string;
  recordedAt:string; notes?:string|null; createdAt:string;
};
type NegotiationEvent = {
  id:string; matterId:string; insuranceClaimId?:string|null;
  eventKind:"call"|"email"|"letter"|"meeting"|"request"|"follow_up"|"other";
  happenedAt:string; summary:string; followUpAt?:string|null; createdAt:string;
};

const CLAIM_STATUS_LABELS:Record<InsuranceClaim["status"],string>={
  open:"פתוח",awaiting_response:"ממתין לתגובה",negotiating:"במו״מ",settled:"הסתיים בהסדר",closed:"סגור"
};
const POSITION_LABELS:Record<NegotiationPosition["kind"],string>={
  demand:"דרישה",offer:"הצעה",counter_offer:"הצעה נגדית"
};
const EVENT_LABELS:Record<NegotiationEvent["eventKind"],string>={
  call:"שיחה",email:"דוא״ל",letter:"מכתב",meeting:"פגישה",request:"בקשה",follow_up:"מעקב",other:"אחר"
};

function localNow(){
  const d=new Date(); d.setMinutes(d.getMinutes()-d.getTimezoneOffset()); return d.toISOString().slice(0,16);
}
function iso(v:string){return new Date(v).toISOString();}
function money(cents:number,currency:string){return (cents/100).toLocaleString("he-IL",{style:"currency",currency});}
function claimLabel(c:InsuranceClaim){return c.claimNumber?c.insurerName+" · "+c.claimNumber:c.insurerName;}

function ClaimsSection({matterId,claims,reload}:{matterId:string;claims:InsuranceClaim[];reload:()=>void}){
  const [open,setOpen]=useState(false);
  const [editing,setEditing]=useState<InsuranceClaim|null>(null);
  const [insurerName,setInsurerName]=useState("");
  const [claimNumber,setClaimNumber]=useState("");
  const [policyNumber,setPolicyNumber]=useState("");
  const [handlerName,setHandlerName]=useState("");
  const [handlerContact,setHandlerContact]=useState("");
  const [status,setStatus]=useState<InsuranceClaim["status"]>("open");
  const [notes,setNotes]=useState("");
  const [busy,setBusy]=useState(false);
  const [err,setErr]=useState<string|null>(null);

  const create=()=>{setEditing(null);setInsurerName("");setClaimNumber("");setPolicyNumber("");setHandlerName("");setHandlerContact("");setStatus("open");setNotes("");setOpen(true);};
  const edit=(c:InsuranceClaim)=>{setEditing(c);setInsurerName(c.insurerName);setClaimNumber(c.claimNumber||"");setPolicyNumber(c.policyNumber||"");setHandlerName(c.handlerName||"");setHandlerContact(c.handlerContact||"");setStatus(c.status);setNotes(c.notes||"");setOpen(true);};
  const save=async()=>{
    if(!insurerName.trim())return;
    setBusy(true);setErr(null);
    try{
      await commands.save_insurance_claim({
        matterId,claimId:editing?.id,insurerName,claimNumber:claimNumber||undefined,
        policyNumber:policyNumber||undefined,handlerName:handlerName||undefined,
        handlerContact:handlerContact||undefined,status,notes:notes||undefined
      });
      setOpen(false);reload();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">INSURANCE</span><h2>תביעות ביטוח</h2></div><button className="btn primary" onClick={create}>תביעה חדשה</button></div>
    <p className="quiet">פרטי המבטחת והמטפל נשמרים ברמת התיק. שינוי ל״הסתיים בהסדר״ הוא רישום אנושי בלבד.</p>
    {claims.length===0&&<p className="quiet">אין עדיין תביעת ביטוח מקושרת לתיק.</p>}
    {claims.map(c=><div className="authority-row" key={c.id}>
      <div><strong>{c.insurerName}</strong><small>{c.claimNumber?"תביעה "+c.claimNumber:"ללא מספר תביעה"}{c.policyNumber?" · פוליסה "+c.policyNumber:""}{c.handlerName?" · "+c.handlerName:""}</small>{c.handlerContact&&<small>{c.handlerContact}</small>}</div>
      <div className="header-actions"><StatusBadge tone={c.status==="settled"||c.status==="closed"?"ok":c.status==="awaiting_response"?"warn":"neutral"}>{CLAIM_STATUS_LABELS[c.status]}</StatusBadge><button className="btn secondary" onClick={()=>edit(c)}>ערוך</button></div>
    </div>)}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(540px,92vw)"}}>
      <h2>{editing?"עריכת תביעת ביטוח":"תביעת ביטוח חדשה"}</h2>
      <label>מבטחת<input autoFocus value={insurerName} onChange={e=>setInsurerName(e.target.value)}/></label>
      <label>מספר תביעה<input value={claimNumber} onChange={e=>setClaimNumber(e.target.value)}/></label>
      <label>מספר פוליסה<input value={policyNumber} onChange={e=>setPolicyNumber(e.target.value)}/></label>
      <label>מטפל/ת<input value={handlerName} onChange={e=>setHandlerName(e.target.value)}/></label>
      <label>פרטי קשר<input value={handlerContact} onChange={e=>setHandlerContact(e.target.value)}/></label>
      <label>סטטוס<select value={status} onChange={e=>setStatus(e.target.value as InsuranceClaim["status"])}>{Object.entries(CLAIM_STATUS_LABELS).map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label>
      <label>הערות<textarea rows={3} value={notes} onChange={e=>setNotes(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={save} disabled={busy||!insurerName.trim()}>{busy?"שומר...":"שמור"}</button></div>
    </div></div>}
  </section>;
}

function PositionsSection({matterId,claims}:{matterId:string;claims:InsuranceClaim[]}){
  const {data,loading,error,reload}=useCommand(()=>commands.list_negotiation_positions({matterId}) as Promise<NegotiationPosition[]>,[matterId]);
  const [open,setOpen]=useState(false); const [claimId,setClaimId]=useState(""); const [side,setSide]=useState<NegotiationPosition["side"]>("counterparty");
  const [kind,setKind]=useState<NegotiationPosition["kind"]>("offer"); const [amount,setAmount]=useState(""); const [recordedAt,setRecordedAt]=useState(localNow());
  const [notes,setNotes]=useState(""); const [busy,setBusy]=useState(false); const [err,setErr]=useState<string|null>(null);
  const rows=data||[];
  const submit=async()=>{
    const amountCents=Math.round(Number(amount)*100); if(!Number.isFinite(amountCents)||amountCents<0)return;
    setBusy(true);setErr(null);
    try{await commands.add_negotiation_position({matterId,insuranceClaimId:claimId||undefined,side,kind,amountCents,currency:"ILS",recordedAt:iso(recordedAt),notes:notes||undefined});setOpen(false);setAmount("");setNotes("");reload();}
    catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">POSITIONS</span><h2>דרישות והצעות</h2></div><button className="btn primary" onClick={()=>{setRecordedAt(localNow());setOpen(true);}}>רשום עמדה</button></div>
    <p className="quiet">היסטוריית הסכומים היא append-only. תיקון נרשם כשורה חדשה; אין כאן פעולה שמקבלת פשרה.</p>
    {loading&&<p className="quiet">טוען...</p>}{error&&<p className="quiet">שגיאה: {error}</p>}{!loading&&!error&&rows.length===0&&<p className="quiet">לא נרשמו עדיין דרישות או הצעות.</p>}
    {rows.map(r=>{const c=claims.find(x=>x.id===r.insuranceClaimId);return <div className="authority-row" key={r.id}><div><strong>{money(r.amountCents,r.currency)}</strong><small>{POSITION_LABELS[r.kind]} · {r.side==="our_side"?"הצד שלנו":"הצד שכנגד"} · {new Date(r.recordedAt).toLocaleString("he-IL")}</small>{c&&<small>{claimLabel(c)}</small>}{r.notes&&<small>{r.notes}</small>}</div><StatusBadge tone={r.side==="counterparty"?"warn":"neutral"}>{POSITION_LABELS[r.kind]}</StatusBadge></div>;})}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(520px,92vw)"}}>
      <h2>רישום דרישה / הצעה</h2>
      <label>תביעת ביטוח<select value={claimId} onChange={e=>setClaimId(e.target.value)}><option value="">ללא קישור</option>{claims.map(c=><option key={c.id} value={c.id}>{claimLabel(c)}</option>)}</select></label>
      <label>צד<select value={side} onChange={e=>setSide(e.target.value as NegotiationPosition["side"])}><option value="our_side">הצד שלנו</option><option value="counterparty">הצד שכנגד</option></select></label>
      <label>סוג<select value={kind} onChange={e=>setKind(e.target.value as NegotiationPosition["kind"])}><option value="demand">דרישה</option><option value="offer">הצעה</option><option value="counter_offer">הצעה נגדית</option></select></label>
      <label>סכום (₪)<input type="number" min="0" step="0.01" value={amount} onChange={e=>setAmount(e.target.value)}/></label>
      <label>מועד<input type="datetime-local" value={recordedAt} onChange={e=>setRecordedAt(e.target.value)}/></label>
      <label>הערות<textarea rows={3} value={notes} onChange={e=>setNotes(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={submit} disabled={busy||!amount.trim()}>{busy?"שומר...":"הוסף להיסטוריה"}</button></div>
    </div></div>}
  </section>;
}

function EventsSection({matterId,claims}:{matterId:string;claims:InsuranceClaim[]}){
  const {data,loading,error,reload}=useCommand(()=>commands.list_negotiation_events({matterId}) as Promise<NegotiationEvent[]>,[matterId]);
  const [open,setOpen]=useState(false); const [claimId,setClaimId]=useState(""); const [eventKind,setEventKind]=useState<NegotiationEvent["eventKind"]>("call");
  const [happenedAt,setHappenedAt]=useState(localNow()); const [summary,setSummary]=useState(""); const [followUpAt,setFollowUpAt]=useState("");
  const [busy,setBusy]=useState(false); const [err,setErr]=useState<string|null>(null); const rows=data||[];
  const submit=async()=>{
    if(!summary.trim())return; setBusy(true);setErr(null);
    try{await commands.add_negotiation_event({matterId,insuranceClaimId:claimId||undefined,eventKind,happenedAt:iso(happenedAt),summary,followUpAt:followUpAt?iso(followUpAt):undefined});setOpen(false);setSummary("");setFollowUpAt("");reload();}
    catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">NEGOTIATION LOG</span><h2>יומן מו״מ</h2></div><button className="btn primary" onClick={()=>{setHappenedAt(localNow());setOpen(true);}}>אירוע חדש</button></div>
    <p className="quiet">שיחות, דוא״ל, מכתבים ומעקבים נשמרים כהיסטוריה תפעולית. המערכת אינה מסיקה מהם הסכמה לפשרה.</p>
    {loading&&<p className="quiet">טוען...</p>}{error&&<p className="quiet">שגיאה: {error}</p>}{!loading&&!error&&rows.length===0&&<p className="quiet">אין עדיין אירועי מו״מ.</p>}
    {rows.map(r=>{const c=claims.find(x=>x.id===r.insuranceClaimId);return <div className="authority-row" key={r.id}><div><strong>{EVENT_LABELS[r.eventKind]}</strong><small>{new Date(r.happenedAt).toLocaleString("he-IL")}{c?" · "+claimLabel(c):""}</small><small>{r.summary}</small>{r.followUpAt&&<small>מעקב: {new Date(r.followUpAt).toLocaleString("he-IL")}</small>}</div>{r.followUpAt&&<StatusBadge tone={new Date(r.followUpAt)<new Date()?"warn":"neutral"}>מעקב</StatusBadge>}</div>;})}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(540px,92vw)"}}>
      <h2>אירוע מו״מ חדש</h2>
      <label>תביעת ביטוח<select value={claimId} onChange={e=>setClaimId(e.target.value)}><option value="">ללא קישור</option>{claims.map(c=><option key={c.id} value={c.id}>{claimLabel(c)}</option>)}</select></label>
      <label>סוג אירוע<select value={eventKind} onChange={e=>setEventKind(e.target.value as NegotiationEvent["eventKind"])}>{Object.entries(EVENT_LABELS).map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label>
      <label>מועד<input type="datetime-local" value={happenedAt} onChange={e=>setHappenedAt(e.target.value)}/></label>
      <label>סיכום<textarea autoFocus rows={4} value={summary} onChange={e=>setSummary(e.target.value)}/></label>
      <label>מעקב הבא<input type="datetime-local" value={followUpAt} onChange={e=>setFollowUpAt(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={submit} disabled={busy||!summary.trim()}>{busy?"שומר...":"הוסף ליומן"}</button></div>
    </div></div>}
  </section>;
}

export function NegotiationTab({matterId}:{matterId:string}){
  const {data,loading,error,reload}=useCommand(()=>commands.list_insurance_claims({matterId}) as Promise<InsuranceClaim[]>,[matterId]);
  const claims=data||[];
  return <div className="matter-tab">
    <div className="workspace-card" style={{marginBottom:16}}><span className="eyebrow">HUMAN CONTROL</span><h2>מו״מ וביטוח</h2><p className="quiet">TAHRIR מתעדת ומארגנת את המו״מ. קבלה או דחייה של פשרה נשארת פעולה אנושית מפורשת.</p></div>
    {loading&&<p className="quiet">טוען...</p>}{error&&<p className="quiet">שגיאה בטעינת תביעות ביטוח: {error}</p>}
    {!loading&&!error&&<><ClaimsSection matterId={matterId} claims={claims} reload={reload}/><PositionsSection matterId={matterId} claims={claims}/><EventsSection matterId={matterId} claims={claims}/></>}
  </div>;
}
