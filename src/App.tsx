import { useEffect, useState } from "react";
import { AppShell, type NavKey } from "./components/AppShell";
import { Inspector } from "./components/Inspector";
import { CommandPalette } from "./components/CommandPalette";
import { TodayPage } from "./pages/TodayPage";
import { MattersPage } from "./pages/MattersPage";
import { ActionCenterPage } from "./pages/ActionCenterPage";
import { CalendarPage } from "./pages/CalendarPage";
import { SearchPage } from "./pages/SearchPage";
import { TemplatesPage } from "./pages/TemplatesPage";
import { AISettingsPage } from "./pages/AISettingsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { MatterWorkspace } from "./pages/MatterWorkspace";

export default function App() {
  const [nav,setNav]=useState<NavKey>("today");
  const [matterId,setMatterId]=useState<string|null>(null);
  const [palette,setPalette]=useState(false);

  useEffect(()=>{
    const h=(e:KeyboardEvent)=>{
      if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==="k"){e.preventDefault();setPalette(true);}
    };
    window.addEventListener("keydown",h);
    return()=>window.removeEventListener("keydown",h);
  },[]);

  const navigate=(target:string)=>{setMatterId(null);setNav(target as NavKey);};
  const page=matterId
    ? <MatterWorkspace matterId={matterId} onBack={()=>setMatterId(null)}/>
    : nav==="today" ? <TodayPage onOpenMatter={setMatterId}/>
    : nav==="matters" ? <MattersPage onOpen={setMatterId}/>
    : nav==="actions" ? <ActionCenterPage onOpenMatter={setMatterId}/>
    : nav==="calendar" ? <CalendarPage/>
    : nav==="search" ? <SearchPage onOpenMatter={setMatterId}/>
    : nav==="templates" ? <TemplatesPage/>
    : nav==="ai" ? <AISettingsPage/>
    : <SettingsPage onNavigate={navigate}/>;

  return <>
    <AppShell active={nav} onNavigate={k=>{setMatterId(null);setNav(k);}} onCommand={()=>setPalette(true)} inspector={<Inspector/>}>{page}</AppShell>
    <CommandPalette open={palette} onClose={()=>setPalette(false)} onNavigate={navigate}/>
  </>;
}
