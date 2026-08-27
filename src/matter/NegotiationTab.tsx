import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { DocumentRow, MatterParty } from "../types";

type ClaimStatus = "open"|"awaiting_response"|"negotiating"|"settled"|"closed";
type InsuranceClaim = {
  id:string; matterId:string; insurerPartyId:string; insurerDisplayName:string; insurerName:string;
  insurerNameSnapshot?:string|null; claimNumber?:string|null; policyNumber?:string|null;
  handlerName?:string|null; handlerContact?:string|null; status:ClaimStatus;
  notes?:string|null; createdAt:string; updatedAt:string;
};
type NegotiationPosition = {
  id:string; matterId:string; insuranceClaimId?:string|null; side:"our_side"|"counterparty";
  kind:"demand"|"offer"|"counter_offer"; amountCents:number; currency:"ILS";
  recordedAt:string; notes?:string|null; sourceDocumentVersionId?:string|null; sourceTitle?:string|null;
  correctedByPositionId?:string|null; correctsPositionId?:string|null; createdAt:string;
};
type NegotiationEvent = {
  id:string; matterId:string; insuranceClaimId?:string|null;
  eventKind:"call"|"email"|"letter"|"meeting"|"request"|"follow_up"|"other";
  happenedAt:string; summary:string; followUpAt?:string|null; operationalFollowUpAt?:string|null;
  waitingForId?:string|null; followUpStatus?:"open"|"closed"|string|null;
  sourceDocumentVersionId?:string|null; sourceTitle?:string|null;
  correctedByEventId?:string|null; correctsEventId?:string|null; createdAt:string;
};
type SnapshotPosition = {id:string; side:string; kind:string; amountCents:number; currency:"ILS"; recordedAt:string};
type NegotiationSnapshot = {
  matterId:string;
  currentClaim?:Pick<InsuranceClaim,"id"|"insurerPartyId"|"insurerDisplayName"|"claimNumber"|"policyNumber"|"handlerName"|"handlerContact"|"status">|null;
  latestOurDemand?:SnapshotPosition|null;
  latestCounterpartyOffer?:SnapshotPosition|null;
  gap?:{amountCents:number; currency:"ILS"}|null;
  latestInteraction?:{id:string; eventKind:NegotiationEvent["eventKind"]; happenedAt:string; summary:string}|null;
  nextFollowUp?:{waitingForId:string; eventId:string; followUpAt:string; overdue:boolean; partyLabel:string; itemLabel:string}|null;
  negotiationStatus?:ClaimStatus|null;
};

const CLAIM_STATUS_LABELS:Record<ClaimStatus,string>={
  open:"פתוח",awaiting_response:"ממתין לתגובה",negotiating:"במו״מ",settled:"הסתיים בהסדר",closed:"סגור"
};
const STATUS_OPTIONS:ClaimStatus[]=["open","awaiting_response","negotiating","settled","closed"];
const POSITION_LABELS:Record<NegotiationPosition["kind"],string>={
  demand:"דרישה",offer:"הצעה",counter_offer:"הצעה נגדית"
};
const EVENT_LABELS:Record<NegotiationEvent["eventKind"],string>={
  call:"שיחה",email:"דוא״ל",letter:"מכתב",meeting:"פגישה",request:"בקשה",follow_up:"מעקב",other:"אחר"
};

function localNow(){
  return toLocalInput(new Date().toISOString());
}
function toLocalInput(value?:string|null){
  if(!value)return "";
  const d=new Date(value);
  if(Number.isNaN(d.getTime()))return "";
  d.setMinutes(d.getMinutes()-d.getTimezoneOffset());
  return d.toISOString().slice(0,16);
}
function toRfc3339(local:string){return new Date(local).toISOString();}
function formatDate(value?:string|null){
  if(!value)return "-";
  const d=new Date(value);
  return Number.isNaN(d.getTime())?value:d.toLocaleString("he-IL");
}
function money(cents?:number|null,currency="ILS"){
  if(cents===undefined||cents===null)return "-";
  return (cents/100).toLocaleString("he-IL",{style:"currency",currency});
}
function parseAmountCents(value:string){
  const clean=value.trim().replace(/,/g,"");
  if(!/^\d+(\.\d{1,2})?$/.test(clean))return null;
  const [whole,frac=""]=clean.split(".");
  const shekels=Number(whole);
  if(!Number.isSafeInteger(shekels))return null;
  return shekels*100+Number(frac.padEnd(2,"0"));
}
function claimLabel(c:InsuranceClaim){return c.claimNumber?`${c.insurerDisplayName} · ${c.claimNumber}`:c.insurerDisplayName;}
function claimTone(status:ClaimStatus){return status==="settled"||status==="closed"?"ok":status==="awaiting_response"?"warn":"neutral";}
function docOptions(documents:DocumentRow[]){return documents.filter(doc=>doc.currentVersionId);}

function SourceSelect({documents,value,onChange}:{documents:DocumentRow[];value:string;onChange:(value:string)=>void}){
  const options=docOptions(documents);
  return <label>מסמך מקור
    <select value={value} onChange={e=>onChange(e.target.value)}>
      <option value="">ללא מקור מסמך</option>
      {options.map(doc=><option key={doc.currentVersionId||doc.id} value={doc.currentVersionId||""}>{doc.fileName} · {doc.category}</option>)}
    </select>
  </label>;
}

function SnapshotPanel({snapshot,loading,error}:{snapshot?:NegotiationSnapshot;loading:boolean;error:string|null}){
  const claim=snapshot?.currentClaim;
  return <section className="workspace-card" style={{marginBottom:16}}>
    <div className="card-head"><div><span className="eyebrow">NEGOTIATION SNAPSHOT</span><h2>תמונת מו״מ</h2></div>{claim&&<StatusBadge tone={claimTone(claim.status)}>{CLAIM_STATUS_LABELS[claim.status]}</StatusBadge>}</div>
    {loading&&<p className="quiet">טוען תמונת מצב...</p>}
    {error&&<p className="quiet">שגיאה בתמונת המו״מ: {error}</p>}
    {!loading&&!error&&<>
      <div className="grid-2">
        <div className="mini-row"><span>מבטחת</span><strong>{claim?.insurerDisplayName??"-"}</strong></div>
        <div className="mini-row"><span>מטפל/ת</span><strong>{claim?.handlerName??"-"}</strong></div>
        <div className="mini-row"><span>דרישה אחרונה</span><strong>{money(snapshot?.latestOurDemand?.amountCents,snapshot?.latestOurDemand?.currency)}</strong></div>
        <div className="mini-row"><span>הצעה אחרונה</span><strong>{money(snapshot?.latestCounterpartyOffer?.amountCents,snapshot?.latestCounterpartyOffer?.currency)}</strong></div>
        <div className="mini-row"><span>פער</span><strong>{snapshot?.gap?money(snapshot.gap.amountCents,snapshot.gap.currency):"-"}</strong></div>
        <div className="mini-row"><span>מעקב הבא</span><strong>{snapshot?.nextFollowUp?formatDate(snapshot.nextFollowUp.followUpAt):"-"}</strong></div>
      </div>
      <div className="next-action" style={{marginTop:12}}>
        <strong>{snapshot?.latestInteraction?EVENT_LABELS[snapshot.latestInteraction.eventKind]:"אין אינטראקציה אחרונה"}</strong>
        <p>{snapshot?.latestInteraction?`${formatDate(snapshot.latestInteraction.happenedAt)} · ${snapshot.latestInteraction.summary}`:"לא נרשם עדיין אירוע מו״מ."}</p>
        {snapshot?.nextFollowUp?.overdue&&<p>מעקב מו״מ תפעולי באיחור: {snapshot.nextFollowUp.itemLabel}</p>}
      </div>
    </>}
  </section>;
}

function ClaimsSection({matterId,claims,parties,reloadAll}:{matterId:string;claims:InsuranceClaim[];parties:MatterParty[];reloadAll:()=>void}){
  const insurers=parties.filter(p=>p.role==="insurer");
  const [open,setOpen]=useState(false);
  const [editing,setEditing]=useState<InsuranceClaim|null>(null);
  const [insurerPartyId,setInsurerPartyId]=useState("");
  const [newInsurerName,setNewInsurerName]=useState("");
  const [claimNumber,setClaimNumber]=useState("");
  const [policyNumber,setPolicyNumber]=useState("");
  const [handlerName,setHandlerName]=useState("");
  const [handlerContact,setHandlerContact]=useState("");
  const [notes,setNotes]=useState("");
  const [transitionClaim,setTransitionClaim]=useState<InsuranceClaim|null>(null);
  const [toStatus,setToStatus]=useState<ClaimStatus>("negotiating");
  const [statusNote,setStatusNote]=useState("");
  const [busy,setBusy]=useState(false);
  const [err,setErr]=useState<string|null>(null);

  const create=()=>{setEditing(null);setInsurerPartyId(insurers[0]?.id??"");setNewInsurerName("");setClaimNumber("");setPolicyNumber("");setHandlerName("");setHandlerContact("");setNotes("");setErr(null);setOpen(true);};
  const edit=(c:InsuranceClaim)=>{setEditing(c);setInsurerPartyId(c.insurerPartyId);setNewInsurerName("");setClaimNumber(c.claimNumber||"");setPolicyNumber(c.policyNumber||"");setHandlerName(c.handlerName||"");setHandlerContact(c.handlerContact||"");setNotes(c.notes||"");setErr(null);setOpen(true);};
  const createInsurer=async()=>{
    if(!newInsurerName.trim())return;
    setBusy(true);setErr(null);
    try{
      const created=await commands.add_matter_party({matterId,role:"insurer",entityKind:"organization",displayName:newInsurerName.trim()}) as {id:string};
      setInsurerPartyId(created.id); setNewInsurerName(""); reloadAll();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  const save=async()=>{
    if(!insurerPartyId)return;
    setBusy(true);setErr(null);
    try{
      await commands.save_insurance_claim({
        matterId,claimId:editing?.id,insurerPartyId,claimNumber:claimNumber||undefined,
        policyNumber:policyNumber||undefined,handlerName:handlerName||undefined,
        handlerContact:handlerContact||undefined,notes:notes||undefined
      });
      setOpen(false);reloadAll();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  const changeStatus=async()=>{
    if(!transitionClaim||transitionClaim.status===toStatus)return;
    setBusy(true);setErr(null);
    try{
      await commands.change_insurance_claim_status({matterId,claimId:transitionClaim.id,toStatus,changedAt:new Date().toISOString(),note:statusNote||undefined,actorKind:"human"});
      setTransitionClaim(null);setStatusNote("");reloadAll();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">INSURANCE</span><h2>תביעות ביטוח</h2></div><button className="btn primary" onClick={create}>תביעה חדשה</button></div>
    <p className="quiet">המבטחת נבחרת מצדדי התיק. שינוי לסטטוס settled או closed נשמר כהחלטה אנושית מפורשת ומתועדת.</p>
    {claims.length===0&&<p className="quiet">אין עדיין תביעת ביטוח מקושרת לתיק.</p>}
    {claims.map(c=><div className="authority-row" key={c.id}>
      <div><strong>{c.insurerDisplayName}</strong><small>{c.claimNumber?"תביעה "+c.claimNumber:"ללא מספר תביעה"}{c.policyNumber?" · פוליסה "+c.policyNumber:""}{c.handlerName?" · "+c.handlerName:""}</small>{c.handlerContact&&<small>{c.handlerContact}</small>}</div>
      <div className="header-actions"><StatusBadge tone={claimTone(c.status)}>{CLAIM_STATUS_LABELS[c.status]}</StatusBadge><button className="btn secondary" onClick={()=>edit(c)}>ערוך</button><button className="btn secondary" onClick={()=>{setTransitionClaim(c);setToStatus(c.status==="settled"?"closed":"negotiating");}}>שנה סטטוס</button></div>
    </div>)}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(560px,92vw)"}}>
      <h2>{editing?"עריכת תביעת ביטוח":"תביעת ביטוח חדשה"}</h2>
      <label>מבטחת קיימת<select value={insurerPartyId} onChange={e=>setInsurerPartyId(e.target.value)}><option value="">בחר מבטחת</option>{insurers.map(p=><option key={p.id} value={p.id}>{p.displayName}</option>)}</select></label>
      <div className="grid-2"><label>הוסף מבטחת חדשה<input value={newInsurerName} onChange={e=>setNewInsurerName(e.target.value)}/></label><button className="btn secondary" style={{alignSelf:"end"}} onClick={createInsurer} disabled={busy||!newInsurerName.trim()}>הוסף לרשימת הצדדים</button></div>
      <label>מספר תביעה<input value={claimNumber} onChange={e=>setClaimNumber(e.target.value)}/></label>
      <label>מספר פוליסה<input value={policyNumber} onChange={e=>setPolicyNumber(e.target.value)}/></label>
      <label>מטפל/ת<input value={handlerName} onChange={e=>setHandlerName(e.target.value)}/></label>
      <label>פרטי קשר<input value={handlerContact} onChange={e=>setHandlerContact(e.target.value)}/></label>
      <label>הערות<textarea rows={3} value={notes} onChange={e=>setNotes(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={save} disabled={busy||!insurerPartyId}>{busy?"שומר...":"שמור"}</button></div>
    </div></div>}
    {transitionClaim&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setTransitionClaim(null);}}><div className="workspace-card" style={{width:"min(520px,92vw)"}}>
      <h2>רישום שינוי סטטוס</h2>
      <p className="quiet">זה רישום של החלטה אנושית. TAHRIR אינה מחליטה לקבל, לדחות או לסגור פשרה.</p>
      <label>סטטוס חדש<select value={toStatus} onChange={e=>setToStatus(e.target.value as ClaimStatus)}>{STATUS_OPTIONS.map(status=><option key={status} value={status}>{CLAIM_STATUS_LABELS[status]}</option>)}</select></label>
      <label>הערת ביקורת<textarea rows={3} value={statusNote} onChange={e=>setStatusNote(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setTransitionClaim(null)} disabled={busy}>ביטול</button><button className="btn primary" onClick={changeStatus} disabled={busy||transitionClaim.status===toStatus}>{busy?"שומר...":"רשום שינוי"}</button></div>
    </div></div>}
  </section>;
}

function PositionsSection({matterId,claims,documents,reloadSnapshot}:{matterId:string;claims:InsuranceClaim[];documents:DocumentRow[];reloadSnapshot:()=>void}){
  const {data,loading,error,reload}=useCommand(()=>commands.list_negotiation_positions(matterId) as Promise<NegotiationPosition[]>,[matterId]);
  const [open,setOpen]=useState(false);
  const [correcting,setCorrecting]=useState<NegotiationPosition|null>(null);
  const [claimId,setClaimId]=useState("");
  const [side,setSide]=useState<NegotiationPosition["side"]>("counterparty");
  const [kind,setKind]=useState<NegotiationPosition["kind"]>("offer");
  const [amount,setAmount]=useState("");
  const [recordedAt,setRecordedAt]=useState(localNow());
  const [notes,setNotes]=useState("");
  const [sourceDocumentVersionId,setSourceDocumentVersionId]=useState("");
  const [reason,setReason]=useState("");
  const [busy,setBusy]=useState(false);
  const [err,setErr]=useState<string|null>(null);
  const rows=data||[];
  const openForm=(row?:NegotiationPosition)=>{
    setCorrecting(row??null);setClaimId(row?.insuranceClaimId??claims[0]?.id??"");setSide(row?.side??"counterparty");
    setKind(row?.kind??"offer");setAmount(row?String(row.amountCents/100):"");setRecordedAt(row?toLocalInput(row.recordedAt):localNow());
    setNotes(row?.notes??"");setSourceDocumentVersionId(row?.sourceDocumentVersionId??"");setReason("");setErr(null);setOpen(true);
  };
  const submit=async()=>{
    const amountCents=parseAmountCents(amount); if(amountCents===null)return;
    setBusy(true);setErr(null);
    const payload={matterId,insuranceClaimId:claimId||undefined,side,kind,amountCents,currency:"ILS",recordedAt:toRfc3339(recordedAt),notes:notes||undefined,sourceDocumentVersionId:sourceDocumentVersionId||undefined};
    try{
      if(correcting){await commands.correct_negotiation_position({...payload,originalPositionId:correcting.id,reason:reason||undefined});}
      else{await commands.add_negotiation_position(payload);}
      setOpen(false);reload();reloadSnapshot();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">POSITIONS</span><h2>דרישות והצעות</h2></div><button className="btn primary" onClick={()=>openForm()}>רשום עמדה</button></div>
    <p className="quiet">היסטוריית הסכומים היא append-only. תיקון נרשם כשורה חדשה עם קשר תיקון פורמלי.</p>
    {loading&&<p className="quiet">טוען...</p>}{error&&<p className="quiet">שגיאה: {error}</p>}{!loading&&!error&&rows.length===0&&<p className="quiet">לא נרשמו עדיין דרישות או הצעות.</p>}
    {rows.map(r=>{const c=claims.find(x=>x.id===r.insuranceClaimId);return <div className="authority-row" key={r.id}><div><strong>{money(r.amountCents,r.currency)}</strong><small>{POSITION_LABELS[r.kind]} · {r.side==="our_side"?"הצד שלנו":"הצד שכנגד"} · {formatDate(r.recordedAt)}</small>{c&&<small>{claimLabel(c)}</small>}{r.sourceTitle&&<small>מקור: {r.sourceTitle}</small>}{r.correctedByPositionId&&<small>תוקן על ידי {r.correctedByPositionId.slice(0,8)}</small>}{r.correctsPositionId&&<small>תיקון של {r.correctsPositionId.slice(0,8)}</small>}{r.notes&&<small>{r.notes}</small>}</div><div className="header-actions"><StatusBadge tone={r.correctedByPositionId?"risk":r.side==="counterparty"?"warn":"neutral"}>{POSITION_LABELS[r.kind]}</StatusBadge><button className="btn secondary" onClick={()=>openForm(r)} disabled={!!r.correctedByPositionId||!!r.correctsPositionId}>תקן</button></div></div>;})}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(560px,92vw)"}}>
      <h2>{correcting?"תיקון עמדה":"רישום דרישה / הצעה"}</h2>
      {correcting&&<p className="quiet">הרשומה המקורית נשארת בהיסטוריה. הרשומה החדשה תהיה הגרסה האפקטיבית.</p>}
      <label>תביעת ביטוח<select value={claimId} onChange={e=>setClaimId(e.target.value)}><option value="">ללא קישור</option>{claims.map(c=><option key={c.id} value={c.id}>{claimLabel(c)}</option>)}</select></label>
      <label>צד<select value={side} onChange={e=>setSide(e.target.value as NegotiationPosition["side"])}><option value="our_side">הצד שלנו</option><option value="counterparty">הצד שכנגד</option></select></label>
      <label>סוג<select value={kind} onChange={e=>setKind(e.target.value as NegotiationPosition["kind"])}><option value="demand">דרישה</option><option value="offer">הצעה</option><option value="counter_offer">הצעה נגדית</option></select></label>
      <label>סכום ILS<input inputMode="decimal" value={amount} onChange={e=>setAmount(e.target.value)} placeholder="0.00"/></label>
      <label>מועד<input type="datetime-local" value={recordedAt} onChange={e=>setRecordedAt(e.target.value)}/></label>
      <SourceSelect documents={documents} value={sourceDocumentVersionId} onChange={setSourceDocumentVersionId}/>
      {correcting&&<label>סיבת תיקון<textarea rows={2} value={reason} onChange={e=>setReason(e.target.value)}/></label>}
      <label>הערות<textarea rows={3} value={notes} onChange={e=>setNotes(e.target.value)}/></label>
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={submit} disabled={busy||parseAmountCents(amount)===null}>{busy?"שומר...":correcting?"שמור תיקון":"הוסף להיסטוריה"}</button></div>
    </div></div>}
  </section>;
}

function EventsSection({matterId,claims,documents,reloadSnapshot}:{matterId:string;claims:InsuranceClaim[];documents:DocumentRow[];reloadSnapshot:()=>void}){
  const {data,loading,error,reload}=useCommand(()=>commands.list_negotiation_events(matterId) as Promise<NegotiationEvent[]>,[matterId]);
  const [open,setOpen]=useState(false);
  const [correcting,setCorrecting]=useState<NegotiationEvent|null>(null);
  const [claimId,setClaimId]=useState("");
  const [eventKind,setEventKind]=useState<NegotiationEvent["eventKind"]>("call");
  const [happenedAt,setHappenedAt]=useState(localNow());
  const [summary,setSummary]=useState("");
  const [followUpAt,setFollowUpAt]=useState("");
  const [sourceDocumentVersionId,setSourceDocumentVersionId]=useState("");
  const [reason,setReason]=useState("");
  const [busy,setBusy]=useState(false);
  const [err,setErr]=useState<string|null>(null);
  const rows=data||[];
  const openForm=(row?:NegotiationEvent)=>{
    setCorrecting(row??null);setClaimId(row?.insuranceClaimId??claims[0]?.id??"");setEventKind(row?.eventKind??"call");
    setHappenedAt(row?toLocalInput(row.happenedAt):localNow());setSummary(row?.summary??"");
    setFollowUpAt(row?.operationalFollowUpAt?toLocalInput(row.operationalFollowUpAt):"");setSourceDocumentVersionId(row?.sourceDocumentVersionId??"");
    setReason("");setErr(null);setOpen(true);
  };
  const submit=async()=>{
    if(!summary.trim())return; setBusy(true);setErr(null);
    const payload={matterId,insuranceClaimId:claimId||undefined,eventKind,happenedAt:toRfc3339(happenedAt),summary,followUpAt:followUpAt?toRfc3339(followUpAt):undefined,sourceDocumentVersionId:sourceDocumentVersionId||undefined};
    try{
      if(correcting){await commands.correct_negotiation_event({...payload,originalEventId:correcting.id,reason:reason||undefined});}
      else{await commands.add_negotiation_event(payload);}
      setOpen(false);reload();reloadSnapshot();
    }catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  const closeFollowUp=async(id:string)=>{
    setBusy(true);setErr(null);
    try{await commands.close_waiting_for({waitingForId:id});reload();reloadSnapshot();}
    catch(e){setErr(String(e));}finally{setBusy(false);}
  };
  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">NEGOTIATION LOG</span><h2>יומן מו״מ</h2></div><button className="btn primary" onClick={()=>openForm()}>אירוע חדש</button></div>
    <p className="quiet">follow-up תפעולי נוצר ומנוהל דרך רשימת ההמתנה של התיק. שדה המעקב ביומן הוא תיעוד היסטורי של מה שהוזן בזמן האירוע.</p>
    {loading&&<p className="quiet">טוען...</p>}{error&&<p className="quiet">שגיאה: {error}</p>}{!loading&&!error&&rows.length===0&&<p className="quiet">אין עדיין אירועי מו״מ.</p>}
    {rows.map(r=>{const c=claims.find(x=>x.id===r.insuranceClaimId);const overdue=!!r.operationalFollowUpAt&&r.followUpStatus==="open"&&new Date(r.operationalFollowUpAt)<new Date();return <div className="authority-row" key={r.id}><div><strong>{EVENT_LABELS[r.eventKind]}</strong><small>{formatDate(r.happenedAt)}{c?" · "+claimLabel(c):""}</small><small>{r.summary}</small>{r.sourceTitle&&<small>מקור: {r.sourceTitle}</small>}{r.operationalFollowUpAt&&<small>מעקב תפעולי: {formatDate(r.operationalFollowUpAt)} · {r.followUpStatus}</small>}{r.correctedByEventId&&<small>תוקן על ידי {r.correctedByEventId.slice(0,8)}</small>}{r.correctsEventId&&<small>תיקון של {r.correctsEventId.slice(0,8)}</small>}</div><div className="header-actions">{r.operationalFollowUpAt&&<StatusBadge tone={overdue?"warn":"neutral"}>{r.followUpStatus==="closed"?"מעקב נסגר":"מעקב פתוח"}</StatusBadge>}{r.waitingForId&&r.followUpStatus==="open"&&<button className="btn secondary" onClick={()=>closeFollowUp(r.waitingForId!)} disabled={busy}>סגור מעקב</button>}<button className="btn secondary" onClick={()=>openForm(r)} disabled={!!r.correctedByEventId||!!r.correctsEventId}>תקן</button></div></div>;})}
    {err&&<p className="quiet">שגיאה: {err}</p>}
    {open&&<div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setOpen(false);}}><div className="workspace-card" style={{width:"min(560px,92vw)"}}>
      <h2>{correcting?"תיקון אירוע מו״מ":"אירוע מו״מ חדש"}</h2>
      {correcting&&<p className="quiet">הרשומה המקורית נשארת בהיסטוריה. אם היה לה מעקב פתוח, התיקון יסגור אותו וייצור מעקב חדש רק אם תבחר מועד.</p>}
      <label>תביעת ביטוח<select value={claimId} onChange={e=>setClaimId(e.target.value)}><option value="">ללא קישור</option>{claims.map(c=><option key={c.id} value={c.id}>{claimLabel(c)}</option>)}</select></label>
      <label>סוג אירוע<select value={eventKind} onChange={e=>setEventKind(e.target.value as NegotiationEvent["eventKind"])}>{Object.entries(EVENT_LABELS).map(([v,l])=><option key={v} value={v}>{l}</option>)}</select></label>
      <label>מועד<input type="datetime-local" value={happenedAt} onChange={e=>setHappenedAt(e.target.value)}/></label>
      <label>סיכום<textarea autoFocus rows={4} value={summary} onChange={e=>setSummary(e.target.value)}/></label>
      <label>מעקב תפעולי הבא<input type="datetime-local" value={followUpAt} onChange={e=>setFollowUpAt(e.target.value)}/></label>
      <SourceSelect documents={documents} value={sourceDocumentVersionId} onChange={setSourceDocumentVersionId}/>
      {correcting&&<label>סיבת תיקון<textarea rows={2} value={reason} onChange={e=>setReason(e.target.value)}/></label>}
      <div className="header-actions"><button className="btn secondary" onClick={()=>setOpen(false)} disabled={busy}>ביטול</button><button className="btn primary" onClick={submit} disabled={busy||!summary.trim()}>{busy?"שומר...":correcting?"שמור תיקון":"הוסף ליומן"}</button></div>
    </div></div>}
  </section>;
}

export function NegotiationTab({matterId}:{matterId:string}){
  const claims=useCommand(()=>commands.list_insurance_claims(matterId) as Promise<InsuranceClaim[]>,[matterId]);
  const parties=useCommand(()=>commands.list_matter_parties({matterId}) as Promise<MatterParty[]>,[matterId]);
  const documents=useCommand(()=>commands.list_documents({matterId}) as Promise<DocumentRow[]>,[matterId]);
  const snapshot=useCommand(()=>commands.get_negotiation_snapshot(matterId) as Promise<NegotiationSnapshot>,[matterId]);
  const reloadAll=()=>{claims.reload();parties.reload();documents.reload();snapshot.reload();};
  const claimRows=claims.data||[];
  return <div className="matter-tab">
    <div className="workspace-card" style={{marginBottom:16}}><span className="eyebrow">HUMAN CONTROL</span><h2>מו״מ וביטוח</h2><p className="quiet">TAHRIR מתעדת, מחשבת פערים ומציפה מעקבים. קבלה, דחייה או סגירת פשרה נשארת פעולה אנושית מפורשת.</p></div>
    <SnapshotPanel snapshot={snapshot.data} loading={snapshot.loading} error={snapshot.error}/>
    {claims.loading||parties.loading||documents.loading?<p className="quiet">טוען סביבת מו״מ...</p>:null}
    {claims.error&&<p className="quiet">שגיאה בטעינת תביעות ביטוח: {claims.error}</p>}
    {parties.error&&<p className="quiet">שגיאה בטעינת צדדים: {parties.error}</p>}
    {documents.error&&<p className="quiet">שגיאה בטעינת מסמכים: {documents.error}</p>}
    {!claims.loading&&!parties.loading&&!documents.loading&&<>
      <ClaimsSection matterId={matterId} claims={claimRows} parties={parties.data||[]} reloadAll={reloadAll}/>
      <PositionsSection matterId={matterId} claims={claimRows} documents={documents.data||[]} reloadSnapshot={snapshot.reload}/>
      <EventsSection matterId={matterId} claims={claimRows} documents={documents.data||[]} reloadSnapshot={snapshot.reload}/>
    </>}
  </div>;
}
