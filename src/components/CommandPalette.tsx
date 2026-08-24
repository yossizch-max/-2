import { useEffect, useRef, useState } from "react";
import type { MouseEvent, ChangeEvent } from "react";

export function CommandPalette({open,onClose,onNavigate}:{open:boolean;onClose:()=>void;onNavigate:(target:string)=>void}) {
  const inputRef=useRef<HTMLInputElement|null>(null);
  const dialogRef=useRef<HTMLDivElement|null>(null);
  const openerRef=useRef<Element|null>(null);
  const [q,setQ]=useState("");

  useEffect(()=>{
    if(open){openerRef.current=document.activeElement;setTimeout(()=>inputRef.current?.focus(),0);}
    else if(openerRef.current instanceof HTMLElement){openerRef.current.focus();}
  },[open]);

  useEffect(()=>{
    if(!open) return;
    const handle=(e:KeyboardEvent)=>{
      if(e.key==="Escape"){e.preventDefault();onClose();return;}
      if(e.key!=="Tab" || !dialogRef.current) return;
      const items=[...dialogRef.current.querySelectorAll<HTMLElement>('button,input,[href],[tabindex]:not([tabindex="-1"])')].filter(x=>!x.hasAttribute("disabled"));
      if(!items.length) return;
      const first=items[0]!, last=items[items.length-1]!;
      if(e.shiftKey && document.activeElement===first){e.preventDefault();last.focus();}
      if(!e.shiftKey && document.activeElement===last){e.preventDefault();first.focus();}
    };
    window.addEventListener("keydown",handle);
    return()=>window.removeEventListener("keydown",handle);
  },[open,onClose]);

  if(!open) return null;
  const actions=[["today","היום"],["matters","פתח תיקים"],["actions","מרכז פעולה"],["search","חיפוש מסמכים"],["ai","הגדרות AI"]];

  return <div className="modal-backdrop" onMouseDown={(e: MouseEvent<HTMLDivElement>)=>{if(e.target===e.currentTarget)onClose();}}>
    <div ref={dialogRef} className="command-palette" role="dialog" aria-modal="true" aria-labelledby="command-title">
      <h2 id="command-title" className="sr-only">חיפוש ופעולות</h2>
      <input ref={inputRef} value={q} onChange={(e: ChangeEvent<HTMLInputElement>)=>setQ(e.target.value)} placeholder="תיק, מסמך או פעולה..." aria-label="חיפוש ופעולות"/>
      <div className="command-results">
        {actions.filter(([,label])=>!q||label.includes(q)).map(([key,label])=>
          <button key={key} onClick={()=>{onNavigate(key);onClose();}}>{label}<span>Enter</span></button>
        )}
      </div>
    </div>
  </div>;
}
