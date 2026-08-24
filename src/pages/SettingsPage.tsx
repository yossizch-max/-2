export function SettingsPage() {
  const rows=[["מסד נתונים","SQLCipher · תקין"],["מפתח שחזור","Restore drill חובה לפני שימוש אמיתי"],["סריקה","Stage A / Stage B · Local-first"],["OCR","Tesseract + Poppler + heb/ara/eng"],["PDF Export","Word/LibreOffice נדרש"],["Code signing","חובה לפני הפצה"]];
  return <div className="page">
    <div className="page-head"><div><span className="eyebrow">SYSTEM</span><h1>הגדרות ובריאות</h1><p>הצפנה, שחזור, סריקה, OCR, Rulesets ו־AI.</p></div></div>
    <div className="settings-list">{rows.map(([a,b])=><div className="setting-row" key={a}><strong>{a}</strong><span>{b}</span></div>)}</div>
  </div>;
}
