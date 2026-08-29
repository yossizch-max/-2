import { useState } from "react";
import type { ReactNode } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import { LEDGER_STATUS_LABELS } from "../types";
import type { DocumentRow, DocumentPage, LedgerSource, MedicalEvent, WageRecord, LiabilityFact } from "../types";

type LedgerKind = "medical" | "wage" | "liability";
type CommonEntry = { id: string; status: string; stale: boolean; superseded: boolean };

function SourceManager({matterId,kind,entryId}:{matterId:string;kind:LedgerKind;entryId:string}) {
  const {data:sources,loading,error,reload}=useCommand(
    ()=>commands.list_ledger_entry_sources({matterId,kind,entryId}) as Promise<LedgerSource[]>, [kind,entryId]
  );
  const {data:documents}=useCommand(
    ()=>commands.list_documents({matterId}) as Promise<DocumentRow[]>, [matterId]
  );
  const sourceableDocuments=documents?.filter(d=>d.currentVersionId)??[];
  const [documentVersionId,setDocumentVersionId]=useState("");
  const {data:pages}=useCommand(
    ()=>documentVersionId?commands.get_document_pages({documentVersionId}) as Promise<DocumentPage[]>:Promise.resolve([]),
    [documentVersionId]
  );
  const [pageId,setPageId]=useState("");
  const [quoteText,setQuoteText]=useState("");
  const [busy,setBusy]=useState(false);
  const [formError,setFormError]=useState<string|null>(null);
  const selectedPage=pages?.find(p=>p.id===pageId);

  const addSource=async()=>{
    if(!pageId||!quoteText.trim())return;
    setBusy(true);setFormError(null);
    try{
      await commands.add_ledger_source({matterId,kind,entryId,sourcePageId:pageId,quoteText});
      setQuoteText("");setPageId("");
      reload();
    }catch(e){ setFormError(String(e)); }
    finally{ setBusy(false); }
  };

  return <div className="workspace-card" style={{marginTop:10}}>
    <h3>מקורות</h3>
    <p className="quiet">אימות הרשומה דורש לפחות מקור אחד, שנבדק מילה-במילה מול טקסט המסמך בתיק.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {sources?.length===0 && <p className="quiet">אין עדיין מקורות.</p>}
    {sources?.map(s=><div className="authority-row" key={s.id}><small>{s.displayQuote}</small></div>)}

    <div style={{marginTop:12}}>
      <label>מסמך
        <select value={documentVersionId} onChange={e=>{setDocumentVersionId(e.target.value);setPageId("");}}>
          <option value="">בחר מסמך...</option>
          {sourceableDocuments.map(d=><option key={d.currentVersionId} value={d.currentVersionId??""}>{d.fileName}</option>)}
        </select>
      </label>
      {documentVersionId && <label>עמוד
        <select value={pageId} onChange={e=>setPageId(e.target.value)}>
          <option value="">בחר עמוד...</option>
          {pages?.map(p=><option key={p.id} value={p.id}>{p.pageNumber?`עמוד ${p.pageNumber}`:p.id}</option>)}
        </select>
      </label>}
      {selectedPage && <blockquote className="source-excerpt"><p>{selectedPage.text}</p></blockquote>}
      <label>ציטוט מדויק מהעמוד (חייב להופיע מילה-במילה)
        <textarea value={quoteText} onChange={e=>setQuoteText(e.target.value)} rows={3}/>
      </label>
      <button className="btn secondary" onClick={addSource} disabled={busy||!pageId||!quoteText.trim()}>
        {busy?"מוסיף...":"הוסף מקור"}
      </button>
    </div>
  </div>;
}

function LedgerEntryRow<T extends CommonEntry>({matterId,kind,entry,summary,managingId,setManagingId,onVerify,onCorrect,busy}:{
  matterId:string; kind:LedgerKind; entry:T; summary:ReactNode;
  managingId:string|null; setManagingId:(id:string|null)=>void;
  onVerify:(id:string)=>void; onCorrect:(entry:T)=>void; busy:string|null;
}) {
  return <div key={entry.id}>
    <div className="authority-row">
      <div>
        {summary}
        {entry.stale && <StatusBadge tone="warn">מקור השתנה</StatusBadge>}
        {entry.superseded && <StatusBadge tone="neutral">הוחלף בתיקון</StatusBadge>}
      </div>
      <div className="header-actions">
        {entry.status==="draft" && <button className="btn secondary" onClick={()=>setManagingId(managingId===entry.id?null:entry.id)}>
          {managingId===entry.id?"סגור מקורות":"נהל מקורות"}
        </button>}
        {entry.status==="draft"
          ? <button className="btn secondary" onClick={()=>onVerify(entry.id)} disabled={busy===entry.id}>{busy===entry.id?"מאמת...":"אמת"}</button>
          : <>
              <StatusBadge tone="ok">{LEDGER_STATUS_LABELS[entry.status]??entry.status}</StatusBadge>
              {!entry.superseded && <button className="btn secondary" onClick={()=>onCorrect(entry)}>תקן</button>}
            </>}
      </div>
    </div>
    {managingId===entry.id && <SourceManager matterId={matterId} kind={kind} entryId={entry.id}/>}
  </div>;
}

// UX Milestone 1: ledger data (verified medical events / wage records / liability
// facts) is never its own "Ledgers" sub-tab inside עבודת התיק - each section is
// exported so it can appear directly inside its own domain's evidence view instead.
export function MedicalSection({matterId}:{matterId:string}) {
  const {data:entries,loading,error,reload}=useCommand(
    ()=>commands.list_medical_events({matterId}) as Promise<MedicalEvent[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [correctingId,setCorrectingId]=useState<string|null>(null);
  const [eventDate,setEventDate]=useState("");
  const [providerName,setProviderName]=useState("");
  const [treatmentSummary,setTreatmentSummary]=useState("");
  const [busy,setBusy]=useState<string|null>(null);
  const [formError,setFormError]=useState<string|null>(null);
  const [managingId,setManagingId]=useState<string|null>(null);

  const openCreate=()=>{setCorrectingId(null);setEventDate("");setProviderName("");setTreatmentSummary("");setCreating(true);};
  const openCorrect=(entry:MedicalEvent)=>{
    setCorrectingId(entry.id);setEventDate(entry.eventDate??"");setProviderName(entry.providerName??"");
    setTreatmentSummary(entry.treatmentSummary);setCreating(true);
  };
  const submit=async()=>{
    if(!treatmentSummary.trim())return;
    setBusy("create");setFormError(null);
    try{
      await commands.create_medical_event({
        matterId,eventDate:eventDate||undefined,providerName:providerName||undefined,
        treatmentSummary,supersedesEntryId:correctingId||undefined,
      });
      setCreating(false);reload();
    }catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };
  const verify=async(entryId:string)=>{
    setBusy(entryId);setFormError(null);
    try{ await commands.verify_ledger_entry({matterId,kind:"medical",entryId}); reload(); }
    catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">MEDICAL</span><h2>אירועים רפואיים</h2></div>
      <button className="btn primary" onClick={openCreate}>אירוע חדש</button></div>
    <p className="quiet">כל רשומה משקפת מה שמסמך מצוטט אומר, לא קביעה משפטית - אימות דורש לפחות מקור אחד.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {!loading && !error && entries?.length===0 && <p className="quiet">אין עדיין אירועים רפואיים בתיק זה.</p>}
    {entries?.map(entry=><LedgerEntryRow key={entry.id} matterId={matterId} kind="medical" entry={entry}
      summary={<div><strong>{entry.eventDate??"—"}</strong><small>{entry.providerName?`${entry.providerName} · `:""}{entry.treatmentSummary}</small></div>}
      managingId={managingId} setManagingId={setManagingId} onVerify={verify} onCorrect={openCorrect} busy={busy}/>)}

    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>{correctingId?"תיקון אירוע רפואי":"אירוע רפואי חדש"}</h2>
        <label>תאריך<input type="date" value={eventDate} onChange={e=>setEventDate(e.target.value)}/></label>
        <label>ספק שירות<input value={providerName} onChange={e=>setProviderName(e.target.value)}/></label>
        <label>תיאור הטיפול<textarea value={treatmentSummary} onChange={e=>setTreatmentSummary(e.target.value)} rows={3}/></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy==="create"}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy==="create"||!treatmentSummary.trim()}>{busy==="create"?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}

export function WageSection({matterId}:{matterId:string}) {
  const {data:entries,loading,error,reload}=useCommand(
    ()=>commands.list_wage_records({matterId}) as Promise<WageRecord[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [correctingId,setCorrectingId]=useState<string|null>(null);
  const [periodStart,setPeriodStart]=useState("");
  const [periodEnd,setPeriodEnd]=useState("");
  const [employerName,setEmployerName]=useState("");
  const [grossAmount,setGrossAmount]=useState("");
  const [busy,setBusy]=useState<string|null>(null);
  const [formError,setFormError]=useState<string|null>(null);
  const [managingId,setManagingId]=useState<string|null>(null);

  const openCreate=()=>{setCorrectingId(null);setPeriodStart("");setPeriodEnd("");setEmployerName("");setGrossAmount("");setCreating(true);};
  const openCorrect=(entry:WageRecord)=>{
    setCorrectingId(entry.id);setPeriodStart(entry.periodStart??"");setPeriodEnd(entry.periodEnd??"");
    setEmployerName(entry.employerName??"");setGrossAmount(String(entry.grossAmountCents/100));setCreating(true);
  };
  const submit=async()=>{
    const cents=Math.round(Number(grossAmount)*100);
    if(!Number.isFinite(cents)||cents<0)return;
    setBusy("create");setFormError(null);
    try{
      await commands.create_wage_record({
        matterId,periodStart:periodStart||undefined,periodEnd:periodEnd||undefined,
        employerName:employerName||undefined,grossAmountCents:cents,supersedesEntryId:correctingId||undefined,
      });
      setCreating(false);reload();
    }catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };
  const verify=async(entryId:string)=>{
    setBusy(entryId);setFormError(null);
    try{ await commands.verify_ledger_entry({matterId,kind:"wage",entryId}); reload(); }
    catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">WAGE</span><h2>רשומות שכר</h2></div>
      <button className="btn primary" onClick={openCreate}>רשומה חדשה</button></div>
    <p className="quiet">כל רשומה משקפת מה שמסמך מצוטט אומר (תלוש שכר, אישור מעסיק), לא חישוב אובדן שכר.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {!loading && !error && entries?.length===0 && <p className="quiet">אין עדיין רשומות שכר בתיק זה.</p>}
    {entries?.map(entry=><LedgerEntryRow key={entry.id} matterId={matterId} kind="wage" entry={entry}
      summary={<div><strong>{(entry.grossAmountCents/100).toLocaleString("he-IL",{style:"currency",currency:"ILS"})}</strong>
        <small>{entry.employerName?`${entry.employerName} · `:""}{entry.periodStart??"—"}{entry.periodEnd?` - ${entry.periodEnd}`:""}</small></div>}
      managingId={managingId} setManagingId={setManagingId} onVerify={verify} onCorrect={openCorrect} busy={busy}/>)}

    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>{correctingId?"תיקון רשומת שכר":"רשומת שכר חדשה"}</h2>
        <label>תחילת תקופה<input type="date" value={periodStart} onChange={e=>setPeriodStart(e.target.value)}/></label>
        <label>סוף תקופה<input type="date" value={periodEnd} onChange={e=>setPeriodEnd(e.target.value)}/></label>
        <label>מעסיק<input value={employerName} onChange={e=>setEmployerName(e.target.value)}/></label>
        <label>סכום ברוטו (₪)<input type="number" step="0.01" value={grossAmount} onChange={e=>setGrossAmount(e.target.value)}/></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy==="create"}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy==="create"||!grossAmount.trim()}>{busy==="create"?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}

export function LiabilitySection({matterId}:{matterId:string}) {
  const {data:entries,loading,error,reload}=useCommand(
    ()=>commands.list_liability_facts({matterId}) as Promise<LiabilityFact[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [correctingId,setCorrectingId]=useState<string|null>(null);
  const [claimBasis,setClaimBasis]=useState("");
  const [liablePartyName,setLiablePartyName]=useState("");
  const [description,setDescription]=useState("");
  const [busy,setBusy]=useState<string|null>(null);
  const [formError,setFormError]=useState<string|null>(null);
  const [managingId,setManagingId]=useState<string|null>(null);

  const openCreate=()=>{setCorrectingId(null);setClaimBasis("");setLiablePartyName("");setDescription("");setCreating(true);};
  const openCorrect=(entry:LiabilityFact)=>{
    setCorrectingId(entry.id);setClaimBasis(entry.claimBasis??"");setLiablePartyName(entry.liablePartyName??"");
    setDescription(entry.description);setCreating(true);
  };
  const submit=async()=>{
    if(!description.trim())return;
    setBusy("create");setFormError(null);
    try{
      await commands.create_liability_fact({
        matterId,claimBasis:claimBasis||undefined,liablePartyName:liablePartyName||undefined,
        description,supersedesEntryId:correctingId||undefined,
      });
      setCreating(false);reload();
    }catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };
  const verify=async(entryId:string)=>{
    setBusy(entryId);setFormError(null);
    try{ await commands.verify_ledger_entry({matterId,kind:"liability",entryId}); reload(); }
    catch(e){ setFormError(String(e)); }
    finally{ setBusy(null); }
  };

  return <section className="workspace-card" style={{marginTop:16}}>
    <div className="card-head"><div><span className="eyebrow">LIABILITY</span><h2>עובדות אחריות</h2></div>
      <button className="btn primary" onClick={openCreate}>רשומה חדשה</button></div>
    <p className="quiet">כל רשומה משקפת מה שמסמך מצוטט אומר (למשל דו"ח משטרה) - לא קביעה משפטית של אחריות.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {formError && <p className="quiet">שגיאה: {formError}</p>}
    {!loading && !error && entries?.length===0 && <p className="quiet">אין עדיין עובדות אחריות בתיק זה.</p>}
    {entries?.map(entry=><LedgerEntryRow key={entry.id} matterId={matterId} kind="liability" entry={entry}
      summary={<div><strong>{entry.liablePartyName??"—"}</strong><small>{entry.claimBasis?`${entry.claimBasis} · `:""}{entry.description}</small></div>}
      managingId={managingId} setManagingId={setManagingId} onVerify={verify} onCorrect={openCorrect} busy={busy}/>)}

    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>{correctingId?"תיקון עובדת אחריות":"עובדת אחריות חדשה"}</h2>
        <label>בסיס הטענה<input value={claimBasis} onChange={e=>setClaimBasis(e.target.value)} placeholder="למשל: רשלנות צד ג׳"/></label>
        <label>הצד הנטען כאחראי<input value={liablePartyName} onChange={e=>setLiablePartyName(e.target.value)}/></label>
        <label>תיאור<textarea value={description} onChange={e=>setDescription(e.target.value)} rows={3}/></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy==="create"}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy==="create"||!description.trim()}>{busy==="create"?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}

export function LedgersTab({matterId}:{matterId:string}) {
  return <div className="matter-tab">
    <MedicalSection matterId={matterId}/>
    <WageSection matterId={matterId}/>
    <LiabilitySection matterId={matterId}/>
  </div>;
}
