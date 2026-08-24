import type { ReactNode } from "react";
export function StatusBadge({tone="neutral",children}:{tone?:"ok"|"warn"|"risk"|"neutral";children:ReactNode}) {
  return <span className={`status-badge ${tone}`}>{children}</span>;
}
