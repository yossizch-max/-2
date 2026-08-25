import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import type { DamageCalculation } from "../types";

const money=(c:number)=>(c/100).toLocaleString("he-IL",{style:"currency",currency:"ILS",maximumFractionDigits:0});
const INPUT_LABELS:Array<[string,string]>=[["past_wage_loss","הפסדי שכר עבר"],["pain_suffering","כאב וסבל"],["deductions","ניכויים"]];

export function DamageTab({matterId}:{matterId:string}) {
  const {data:calculations,loading,error,reload}=useCommand(
    ()=>commands.list_damage_calculations({matterId}) as Promise<DamageCalculation[]>, [matterId]
  );
  const [creating,setCreating]=useState(false);
  const [regime,setRegime]=useState<"pip"|"tort">("tort");
  const [lifeState,setLifeState]=useState<"living"|"death">("living");
  const [amounts,setAmounts]=useState<Record<string,string>>({});
  const [busy,setBusy]=useState(false);
  const [formError,setFormError]=useState<string|null>(null);

  const c=calculations?.[0];

  const submit=async()=>{
    setBusy(true);setFormError(null);
    try{
      const inputs=INPUT_LABELS
        .map(([key])=>({key,cents:Math.round(Number(amounts[key]||0)*100),source:"manual"}))
        .filter(i=>i.cents>0);
      await commands.save_damage_calculation({matterId,regime,lifeState,inputs});
      setCreating(false);setAmounts({});
      reload();
    }catch(e){setFormError(String(e));}
    finally{setBusy(false);}
  };

  const lock=async()=>{
    if(!c||c.status!=="draft")return;
    setBusy(true);setFormError(null);
    try{
      const calc=await commands.calculate_damage({regime:c.regime,lifeState:c.lifeState,inputs:c.inputs??[]}) as {integritySha256:string};
      await commands.lock_damage_calculation({calculationId:c.id,integritySha256:calc.integritySha256});
      reload();
    }catch(e){setFormError(String(e));}
    finally{setBusy(false);}
  };

  return <div>
    <section className="workspace-card">
      <div className="card-head"><div><span className="eyebrow">DETERMINISTIC ENGINE</span><h2>תחשיב נזק</h2></div>
        {c && <span className={`status-badge ${c.status==="locked"?"ok":""}`}>{c.status}</span>}
        <button className="btn secondary" onClick={()=>setCreating(true)}>תחשיב חדש</button>
      </div>
      {loading && <p className="quiet">טוען...</p>}
      {error && <p className="quiet">שגיאה: {error}</p>}
      {!loading && !error && !c && <p className="quiet">אין עדיין תחשיב נזק בתיק זה.</p>}
      {c && <div className="damage-summary">
        <div><span>ברוטו</span><strong>{money(c.grossCents)}</strong></div>
        <div><span>ניכויים</span><strong>{money(c.deductionsCents)}</strong></div>
        <div className="net"><span>נטו</span><strong>{money(c.netCents)}</strong></div>
      </div>}
      {formError && <p className="quiet">{formError}</p>}
      {c?.status==="draft" &&
        <button className="btn primary" onClick={lock} disabled={busy}>{busy?"נועל...":"נעל תחשיב (בלתי הפיך)"}</button>}
      <div className="legal-note"><strong>כלל</strong><span>כסף נשמר באגורות. AI רשאי להציע inputs, אך אינו משנה נוסחה או Ruleset.</span></div>
    </section>

    {creating && <div className="modal-backdrop" onMouseDown={(e)=>{if(e.target===e.currentTarget)setCreating(false);}}>
      <div className="workspace-card" style={{width:"min(480px,90vw)"}}>
        <h2>תחשיב נזק חדש</h2>
        <label>משטר<select value={regime} onChange={e=>setRegime(e.target.value as "pip"|"tort")}><option value="tort">נזיקין</option><option value="pip">ביטוח חובה (PIP)</option></select></label>
        <label>מצב<select value={lifeState} onChange={e=>setLifeState(e.target.value as "living"|"death")}><option value="living">נפגע בחיים</option><option value="death">עיזבון</option></select></label>
        {INPUT_LABELS.map(([key,label])=>
          <label key={key}>{label} (₪)<input type="number" min="0" value={amounts[key]??""} onChange={e=>setAmounts(a=>({...a,[key]:e.target.value}))}/></label>
        )}
        {formError && <p className="quiet">{formError}</p>}
        <div className="header-actions">
          <button className="btn secondary" onClick={()=>setCreating(false)} disabled={busy}>ביטול</button>
          <button className="btn primary" onClick={submit} disabled={busy}>{busy?"שומר...":"שמור טיוטה"}</button>
        </div>
      </div>
    </div>}

    <section className="workspace-card"><h2>מקורות קלט</h2>
      {c?.inputs && c.inputs.length>0
        ? <div className="table">
            <div className="tr th"><span>ראש נזק</span><span>ערך</span><span>מקור</span></div>
            {c.inputs.map((i,idx)=><div className="tr" key={idx}><span>{i.key}</span><span>{money(i.cents)}</span><span>{i.source}</span></div>)}
          </div>
        : <p className="quiet">אין עדיין קלטים.</p>}
    </section>
  </div>;
}
