import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { PARTY_ROLES, ENTITY_KINDS, type Matter, type DocumentRow, type Deadline, type MatterProfile, type MatterParty } from "../types";
import { CaseHealthPanel } from "./CaseHealthPanel";

export function OverviewTab({matter}:{matter:Matter}) {
  const {data:documents}=useCommand(
    ()=>commands.list_documents({matterId:matter.id}) as Promise<DocumentRow[]>, [matter.id]
  );
  const {data:deadlines}=useCommand(
    ()=>commands.list_deadlines({matterId:matter.id}) as Promise<Deadline[]>, [matter.id]
  );
  const {data:profile,reload:reloadProfile}=useCommand(
    ()=>commands.get_matter_profile({matterId:matter.id}) as Promise<MatterProfile>, [matter.id]
  );
  const {data:parties,reload:reloadParties}=useCommand(
    ()=>commands.list_matter_parties({matterId:matter.id}) as Promise<MatterParty[]>, [matter.id]
  );
  const nextDeadline=deadlines?.filter(d=>d.state==="committed")[0];

  const [editingProfile,setEditingProfile]=useState(false);
  const [primaryEventDate,setPrimaryEventDate]=useState("");
  const [primaryCourtName,setPrimaryCourtName]=useState("");
  const [btlClaimNumber,setBtlClaimNumber]=useState("");
  const [caseSummary,setCaseSummary]=useState("");
  const [profileBusy,setProfileBusy]=useState(false);
  const [profileError,setProfileError]=useState<string|null>(null);

  const openProfileEditor=()=>{
    setPrimaryEventDate(profile?.primaryEventDate??"");
    setPrimaryCourtName(profile?.primaryCourtName??"");
    setBtlClaimNumber(profile?.btlClaimNumber??"");
    setCaseSummary(profile?.caseSummary??"");
    setProfileError(null);
    setEditingProfile(true);
  };
  const saveProfile=async()=>{
    setProfileBusy(true);setProfileError(null);
    try{
      await commands.save_matter_profile({
        matterId:matter.id,
        primaryEventDate:primaryEventDate||undefined, primaryCourtName:primaryCourtName||undefined,
        btlClaimNumber:btlClaimNumber||undefined, caseSummary:caseSummary||undefined,
      });
      setEditingProfile(false);
      reloadProfile();
    }catch(e){setProfileError(String(e));}
    finally{setProfileBusy(false);}
  };

  const [editingParty,setEditingParty]=useState<MatterParty|null|"new">(null);
  const [partyRole,setPartyRole]=useState("client");
  const [partyDisplayName,setPartyDisplayName]=useState("");
  const [partyEntityKind,setPartyEntityKind]=useState("unknown");
  const [partyIdentifier,setPartyIdentifier]=useState("");
  const [partyPhone,setPartyPhone]=useState("");
  const [partyEmail,setPartyEmail]=useState("");
  const [partyAddress,setPartyAddress]=useState("");
  const [partyNotes,setPartyNotes]=useState("");
  const [partyBusy,setPartyBusy]=useState(false);
  const [partyError,setPartyError]=useState<string|null>(null);

  const openNewParty=()=>{
    setPartyRole("client");setPartyDisplayName("");setPartyEntityKind("unknown");
    setPartyIdentifier("");setPartyPhone("");setPartyEmail("");setPartyAddress("");setPartyNotes("");
    setPartyError(null);setEditingParty("new");
  };
  const openEditParty=(p:MatterParty)=>{
    setPartyRole(p.role);setPartyDisplayName(p.displayName);setPartyEntityKind(p.entityKind);
    setPartyIdentifier(p.identifier??"");setPartyPhone(p.phone??"");setPartyEmail(p.email??"");
    setPartyAddress(p.address??"");setPartyNotes(p.notes??"");
    setPartyError(null);setEditingParty(p);
  };
  const saveParty=async()=>{
    if(!partyDisplayName.trim())return;
    setPartyBusy(true);setPartyError(null);
    try{
      const fields={
        role:partyRole, displayName:partyDisplayName, entityKind:partyEntityKind,
        identifier:partyIdentifier||undefined, phone:partyPhone||undefined,
        email:partyEmail||undefined, address:partyAddress||undefined, notes:partyNotes||undefined,
      };
      if(editingParty==="new"){
        await commands.add_matter_party({matterId:matter.id, ...fields});
      }else if(editingParty){
        await commands.update_matter_party({partyId:editingParty.id, matterId:matter.id, ...fields});
      }
      setEditingParty(null);
      reloadParties();
    }catch(e){setPartyError(String(e));}
    finally{setPartyBusy(false);}
  };
  const deleteParty=async()=>{
    if(editingParty==="new"||!editingParty)return;
    setPartyBusy(true);setPartyError(null);
    try{
      await commands.delete_matter_party({partyId:editingParty.id, matterId:matter.id});
      setEditingParty(null);
      reloadParties();
    }catch(e){setPartyError(String(e));}
    finally{setPartyBusy(false);}
  };
  const roleLabel=(v:string)=>PARTY_ROLES.find(r=>r.value===v)?.label??v;

  return <div className="matter-tab">
    <CaseHealthPanel matterId={matter.id}/>
    <div className="kpi-grid">
      <div className="kpi-tile"><span>מסמכים</span><strong>{matter.documentCount}</strong><small>ראו בלשונית מסמכים</small></div>
      <div className="kpi-tile"><span>עובדות</span><strong>{matter.verifiedFactCount}</strong><small>{matter.pendingReviewCount} לבדיקה</small></div>
      <div className="kpi-tile"><span>מועד קרוב</span><strong>{nextDeadline?.dueAt ?? "—"}</strong><small>מחייב רק לאחר commit</small></div>
      <div className="kpi-tile"><span>שלב</span><strong>{matter.workflowStage}</strong><small>המעבר מאושר בידי המשתמש</small></div>
    </div>
    <section className="workspace-card"><h2>מסמכים אחרונים</h2>
      {documents?.length
        ? documents.slice(0,3).map(d=><button className="mini-row" key={d.id} disabled={!d.occurrenceId}
            onClick={()=>d.occurrenceId&&commands.open_occurrence({occurrenceId:d.occurrenceId})}>
            <span>{d.fileName}</span><small>{d.category} · {d.extractionState}</small></button>)
        : <p className="quiet">אין עדיין מסמכים. סרקו את תיקיית התיק בלשונית מסמכים.</p>}
    </section>
    <div className="grid-2">
      <section className="workspace-card">
        <div className="header-actions" style={{justifyContent:"space-between"}}><h2>פרופיל תיק</h2><button className="btn secondary" onClick={openProfileEditor}>ערוך</button></div>
        {profile?.updatedAt
          ? <dl className="profile-fields">
              <div><dt>תאריך אירוע יסודי</dt><dd>{profile.primaryEventDate??"—"}</dd></div>
              <div><dt>בית משפט עיקרי</dt><dd>{profile.primaryCourtName??"—"}</dd></div>
              <div><dt>מספר תביעה במל"ל</dt><dd>{profile.btlClaimNumber??"—"}</dd></div>
              <div><dt>תקציר תיק</dt><dd>{profile.caseSummary??"—"}</dd></div>
            </dl>
          : <p className="quiet">עדיין לא הוזן פרופיל תיק.</p>}
      </section>
      <section className="workspace-card">
        <div className="header-actions" style={{justifyContent:"space-between"}}><h2>צדדים</h2><button className="btn secondary" onClick={openNewParty}>הוסף צד</button></div>
        {parties?.length
          ? parties.map(p=><button className="mini-row" key={p.id} onClick={()=>openEditParty(p)}>
              <span>{p.displayName}</span><small>{roleLabel(p.role)}{p.phone?` · ${p.phone}`:""}{p.email?` · ${p.email}`:""}</small>
            </button>)
          : <p className="quiet">אין עדיין צדדים רשומים בתיק.</p>}
      </section>
    </div>
    {editingProfile && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setEditingProfile(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>עריכת פרופיל תיק</h2>
        <label>תאריך אירוע יסודי<input type="date" value={primaryEventDate} onChange={e=>setPrimaryEventDate(e.target.value)}/></label>
        <label>בית משפט עיקרי<input value={primaryCourtName} onChange={e=>setPrimaryCourtName(e.target.value)}/></label>
        <label>מספר תביעה במל"ל<input value={btlClaimNumber} onChange={e=>setBtlClaimNumber(e.target.value)}/></label>
        <label>תקציר תיק<textarea value={caseSummary} onChange={e=>setCaseSummary(e.target.value)} rows={4}/></label>
        {profileError && <p className="quiet">{profileError}</p>}
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setEditingProfile(false)} disabled={profileBusy}>ביטול</button>
          <button className="btn primary" onClick={saveProfile} disabled={profileBusy}>{profileBusy?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
    {editingParty && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setEditingParty(null);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>{editingParty==="new"?"צד חדש":"עריכת צד"}</h2>
        <label>תפקיד<select value={partyRole} onChange={e=>setPartyRole(e.target.value)}>
          {PARTY_ROLES.map(r=><option key={r.value} value={r.value}>{r.label}</option>)}
        </select></label>
        <label>שם תצוגה<input autoFocus value={partyDisplayName} onChange={e=>setPartyDisplayName(e.target.value)}/></label>
        <label>סוג ישות<select value={partyEntityKind} onChange={e=>setPartyEntityKind(e.target.value)}>
          {ENTITY_KINDS.map(k=><option key={k.value} value={k.value}>{k.label}</option>)}
        </select></label>
        <label>מספר זהות/ח.פ (אופציונלי)<input value={partyIdentifier} onChange={e=>setPartyIdentifier(e.target.value)}/></label>
        <label>טלפון<input value={partyPhone} onChange={e=>setPartyPhone(e.target.value)}/></label>
        <label>אימייל<input value={partyEmail} onChange={e=>setPartyEmail(e.target.value)}/></label>
        <label>כתובת<input value={partyAddress} onChange={e=>setPartyAddress(e.target.value)}/></label>
        <label>הערות<textarea value={partyNotes} onChange={e=>setPartyNotes(e.target.value)} rows={3}/></label>
        {partyError && <p className="quiet">{partyError}</p>}
        <div className="header-actions">
          {editingParty!=="new" && <button className="btn secondary" onClick={deleteParty} disabled={partyBusy}>מחק</button>}
          <button className="btn secondary" onClick={()=>setEditingParty(null)} disabled={partyBusy}>ביטול</button>
          <button className="btn primary" onClick={saveParty} disabled={partyBusy||!partyDisplayName.trim()}>{partyBusy?"שומר...":"שמור"}</button>
        </div>
      </div>
    </div>}
  </div>;
}
