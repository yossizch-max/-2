import { useCallback, useEffect, useState } from "react";

export function useCommand<T>(fn:()=>Promise<T>, deps:unknown[]): {
  data:T|undefined; loading:boolean; error:string|null; reload:()=>void;
} {
  const [data,setData]=useState<T|undefined>(undefined);
  const [loading,setLoading]=useState(true);
  const [error,setError]=useState<string|null>(null);
  const [tick,setTick]=useState(0);

  useEffect(()=>{
    let cancelled=false;
    setLoading(true); setError(null);
    fn().then(result=>{if(!cancelled){setData(result);setLoading(false);}})
        .catch(err=>{if(!cancelled){setError(String(err));setLoading(false);}});
    return()=>{cancelled=true;};
    // eslint-disable-next-line react-hooks/exhaustive-deps
  },[...deps,tick]);

  const reload=useCallback(()=>setTick(t=>t+1),[]);
  return {data,loading,error,reload};
}
