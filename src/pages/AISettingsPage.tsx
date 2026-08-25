import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";

type AiProfile={id:string;providerKind:string;baseUrl:string;model?:string|null;enabled:boolean;clientDataAuthorized:boolean};

function ProviderCard({kind,label,defaultBaseUrl,profile,reload}:{
  kind:"local"|"openai"; label:string; defaultBaseUrl:string; profile?:AiProfile; reload:()=>void;
}) {
  const [baseUrl,setBaseUrl]=useState(profile?.baseUrl ?? defaultBaseUrl);
  const [model,setModel]=useState(profile?.model ?? "");
  const [apiKey,setApiKey]=useState("");
  const [clientDataAuthorized,setClientDataAuthorized]=useState(profile?.clientDataAuthorized??false);
  const [busy,setBusy]=useState(false);
  const [status,setStatus]=useState<string|null>(null);

  const save=async(enabled:boolean)=>{
    setBusy(true);setStatus(null);
    try{
      await commands.save_ai_settings({
        id:profile?.id, providerKind:kind, baseUrl, model:model||undefined,
        enabled, clientDataAuthorized,
        apiKey:apiKey||undefined,
      });
      setApiKey("");
      reload();
    }catch(e){setStatus(String(e));}
    finally{setBusy(false);}
  };
  const test=async()=>{
    if(!profile){setStatus("יש לשמור הגדרות לפני בדיקה.");return;}
    setBusy(true);setStatus(null);
    try{
      const res=await commands.test_ai_provider({profileId:profile.id}) as {ok:boolean};
      setStatus(res.ok?"תקין":"נכשל");
    }catch(e){setStatus(String(e));}
    finally{setBusy(false);}
  };

  return <section className="workspace-card">
    <div className="card-head"><h2>{label}</h2>
      {kind==="local" && <StatusBadge tone="ok">Loopback only</StatusBadge>}
      {kind==="openai" && <StatusBadge tone={profile?.clientDataAuthorized?"ok":"warn"}>{profile?.clientDataAuthorized?"מאושר לחומר לקוח":"לא מאושר לחומר לקוח"}</StatusBadge>}
    </div>
    <label>Endpoint<input value={baseUrl} onChange={e=>setBaseUrl(e.target.value)} readOnly={kind==="openai"}/></label>
    <label>Model<input value={model??""} onChange={e=>setModel(e.target.value)} placeholder={kind==="local"?"llama3":"gpt-4.1"}/></label>
    {kind==="openai" && <label>API Key<input type="password" value={apiKey} onChange={e=>setApiKey(e.target.value)} placeholder={profile?"••••••••":"הזן מפתח"}/></label>}
    {kind==="openai" && <label style={{display:"flex",alignItems:"center",gap:8,flexDirection:"row"}}>
      <input type="checkbox" checked={clientDataAuthorized} onChange={e=>setClientDataAuthorized(e.target.checked)}/>
      מאשר שליחת חומר מהתיק לספק חיצוני זה (נדרש לפני כל הרצת AI על נתוני לקוח)
    </label>}
    {status && <p className="quiet">{status}</p>}
    <div className="header-actions">
      <button className="btn secondary" onClick={test} disabled={busy}>בדיקה סינתטית</button>
      <button className="btn secondary" onClick={()=>save(profile?.enabled??false)} disabled={busy}>שמור</button>
      <button className="btn primary" onClick={()=>save(!(profile?.enabled))} disabled={busy}>
        {profile?.enabled?"השבת":"הפעל"}
      </button>
    </div>
  </section>;
}

export function AISettingsPage() {
  const {data:profiles,reload}=useCommand(
    ()=>commands.get_ai_settings() as Promise<AiProfile[]>, []
  );
  const local=profiles?.find(p=>p.providerKind==="local");
  const openai=profiles?.find(p=>p.providerKind==="openai");

  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">AI CONTROL</span><h1>AI</h1><p>חיבור טכני ואישור נתוני לקוח הם שני שערים נפרדים.</p></div></div>
    <div className="grid-2">
      <ProviderCard kind="local" label="Local compatible" defaultBaseUrl="http://127.0.0.1:11434/v1" profile={local} reload={reload}/>
      <ProviderCard kind="openai" label="OpenAI" defaultBaseUrl="https://api.openai.com/v1" profile={openai} reload={reload}/>
    </div>
    <section className="workspace-card"><h2>Client Data Gate</h2><p className="warning-box">חיבור תקין לספק אינו מרשה שליחת חומר מתיק. כל egress חיצוני דורש הרשאה מפורשת.</p></section>
  </div>;
}
