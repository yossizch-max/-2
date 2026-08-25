import { useState } from "react";
import { commands } from "../lib/ipc";
import { useCommand } from "../lib/hooks";
import { StatusBadge } from "../components/StatusBadge";
import type { LegalRuleset, LegalRulesetDetail } from "../types";

const STATUS_LABEL: Record<string, string> = {
  draft: "טיוטה", under_review: "בבדיקה", approved: "מאושר", superseded: "הוחלף", revoked: "בוטל",
};
const STATUS_TONE: Record<string, "ok" | "warn" | "risk" | "neutral"> = {
  draft: "neutral", under_review: "warn", approved: "ok", superseded: "neutral", revoked: "risk",
};

function RulesetList({ onOpen }: { onOpen: (id: string) => void }) {
  const { data: rulesets, loading, error, reload } = useCommand(
    () => commands.list_legal_rulesets({}) as Promise<LegalRuleset[]>, []
  );
  const [creating, setCreating] = useState(false);
  const [engineKind, setEngineKind] = useState("deadline");
  const [jurisdiction, setJurisdiction] = useState("IL");
  const [title, setTitle] = useState("");
  const [version, setVersion] = useState("1");
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const create = async () => {
    if (!title.trim()) return;
    setBusy(true); setFormError(null);
    try {
      const res = await commands.create_legal_ruleset({ engineKind, jurisdiction, title, version }) as { id: string };
      setCreating(false); setTitle("");
      reload();
      onOpen(res.id);
    } catch (e) { setFormError(String(e)); }
    finally { setBusy(false); }
  };

  return <section className="workspace-card">
    <div className="card-head"><div><span className="eyebrow">GOVERNANCE</span><h2>Rulesets</h2></div>
      <button className="btn primary" onClick={() => setCreating(true)}>Ruleset חדש</button></div>
    <p className="quiet">כלל משפטי דטרמיניסטי משמש לחישוב מועד או פיצוי רק אם הוא חלק מ-Ruleset מאושר: עם מקור מאומת ובדיקות שעוברות. לטיוטה אין כפתור הפעלה.</p>
    {loading && <p className="quiet">טוען...</p>}
    {error && <p className="quiet">שגיאה: {error}</p>}
    {!loading && !error && rulesets?.length === 0 && <p className="quiet">אין עדיין Rulesets.</p>}
    {rulesets && rulesets.length > 0 && <div className="table rulesets-table">
      <div className="tr th"><span>מנוע / כותרת</span><span>גרסה</span><span>סטטוס</span><span>מקורות</span><span>בדיקות מאושרות</span></div>
      {rulesets.map(r => <div className="tr" key={r.id} onClick={() => onOpen(r.id)} style={{ cursor: "pointer" }}>
        <span><b>{r.title}</b><small>{r.engineKind} · {r.jurisdiction}</small></span>
        <span>{r.version}</span>
        <span><StatusBadge tone={STATUS_TONE[r.status]}>{STATUS_LABEL[r.status]}</StatusBadge></span>
        <span>{r.sourceCount}</span>
        <span>{r.approvedTestCaseCount}/{r.testCaseCount}</span>
      </div>)}
    </div>}
    {creating && <div className="modal-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) setCreating(false); }}>
      <div className="workspace-card" style={{ width: "min(480px,90vw)" }}>
        <h2>Ruleset חדש</h2>
        {formError && <p className="quiet">שגיאה: {formError}</p>}
        <label>מנוע<select value={engineKind} onChange={e => setEngineKind(e.target.value)}>
          <option value="deadline">deadline</option><option value="damage">damage</option>
        </select></label>
        <label>תחום שיפוט<input value={jurisdiction} onChange={e => setJurisdiction(e.target.value)} /></label>
        <label>כותרת<input autoFocus value={title} onChange={e => setTitle(e.target.value)} /></label>
        <label>גרסה<input value={version} onChange={e => setVersion(e.target.value)} /></label>
        <div className="header-actions">
          <button className="btn secondary" onClick={() => setCreating(false)} disabled={busy}>ביטול</button>
          <button className="btn primary" onClick={create} disabled={busy || !title.trim()}>{busy ? "יוצר..." : "צור"}</button>
        </div>
      </div>
    </div>}
  </section>;
}

type TestResult = { name: string; passed: boolean; detail: string };

function RulesetEditor({ rulesetId, onBack }: { rulesetId: string; onBack: () => void }) {
  const { data: rs, loading, error, reload } = useCommand(
    () => commands.get_legal_ruleset({ rulesetId }) as Promise<LegalRulesetDetail>, [rulesetId]
  );
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const [sourceKind, setSourceKind] = useState("legislation");
  const [citation, setCitation] = useState("");
  const [pinpoint, setPinpoint] = useState("");
  const [verifiedBy, setVerifiedBy] = useState("");
  const addSource = async () => {
    if (!citation.trim()) return;
    setBusy("source"); setActionError(null);
    try {
      await commands.add_legal_ruleset_source({
        rulesetId, sourceKind, citation, pinpoint: pinpoint || undefined, verifiedBy: verifiedBy || undefined
      });
      setCitation(""); setPinpoint(""); setVerifiedBy("");
      reload();
    } catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const [ruleKey, setRuleKey] = useState("");
  const [priority, setPriority] = useState("0");
  const [conditionsText, setConditionsText] = useState('[{"field":"","op":"eq","value":""}]');
  const [operationText, setOperationText] = useState('[{"op":"add_days","from":{"reg":"trigger_date"},"days":30,"into":"result"}]');
  const [explanationTemplate, setExplanationTemplate] = useState("");
  const [ruleSourceId, setRuleSourceId] = useState("");
  const addRule = async () => {
    if (!ruleKey.trim()) return;
    setBusy("rule"); setActionError(null);
    try {
      const conditions = JSON.parse(conditionsText);
      const operation = JSON.parse(operationText);
      await commands.add_legal_rule({
        rulesetId, ruleKey, ruleType: "deterministic", priority: Number(priority) || 0,
        conditions, operation, explanationTemplate: explanationTemplate || undefined, sourceId: ruleSourceId || undefined
      });
      setRuleKey("");
      reload();
    } catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const [tcName, setTcName] = useState("");
  const [tcInput, setTcInput] = useState('{"trigger_date":"2026-01-01"}');
  const [tcExpected, setTcExpected] = useState('{"result":"..."}');
  const addTestCase = async () => {
    if (!tcName.trim()) return;
    setBusy("tc"); setActionError(null);
    try {
      const input = JSON.parse(tcInput);
      const expectedOutput = JSON.parse(tcExpected);
      await commands.add_legal_rule_test_case({ rulesetId, name: tcName, input, expectedOutput });
      setTcName("");
      reload();
    } catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const reviewTestCase = async (testCaseId: string, approved: boolean) => {
    setBusy(testCaseId); setActionError(null);
    try { await commands.review_legal_rule_test_case({ rulesetId, testCaseId, approved }); reload(); }
    catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const [testResults, setTestResults] = useState<TestResult[] | null>(null);
  const runTests = async () => {
    setBusy("run"); setActionError(null);
    try { setTestResults(await commands.run_legal_rule_tests({ rulesetId }) as TestResult[]); }
    catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const submit = async () => {
    setBusy("submit"); setActionError(null);
    try { await commands.submit_legal_ruleset_for_review({ rulesetId }); reload(); }
    catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };
  const approve = async () => {
    setBusy("approve"); setActionError(null);
    try { await commands.approve_legal_ruleset({ rulesetId }); reload(); }
    catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  const [previewContext, setPreviewContext] = useState('{"procedure_type":"","trigger_date":"2026-01-01"}');
  const [previewResult, setPreviewResult] = useState<{ matchedRuleKey: string; explanation: string } | null>(null);
  const runPreview = async () => {
    setBusy("preview"); setActionError(null); setPreviewResult(null);
    try {
      const context = JSON.parse(previewContext);
      setPreviewResult(await commands.preview_legal_engine_run({ rulesetId, context }) as { matchedRuleKey: string; explanation: string });
    } catch (e) { setActionError(String(e)); }
    finally { setBusy(null); }
  };

  if (loading) return <p className="quiet">טוען...</p>;
  if (error || !rs) return <p className="quiet">שגיאה: {error}</p>;
  const isDraft = rs.status === "draft";

  return <div>
    <button className="back-link" onClick={onBack}>‹ חזרה ל-Rulesets</button>
    <div className="matter-header">
      <h1>{rs.title} <small>v{rs.version}</small></h1>
      <div className="meta-chips">
        <span>{rs.engineKind}</span><span>{rs.jurisdiction}</span>
        <StatusBadge tone={STATUS_TONE[rs.status]}>{STATUS_LABEL[rs.status]}</StatusBadge>
      </div>
      <div className="header-actions">
        {isDraft && <button className="btn secondary" onClick={submit} disabled={busy === "submit"}>{busy === "submit" ? "שולח..." : "שלח לבדיקה"}</button>}
        {(rs.status === "draft" || rs.status === "under_review") &&
          <button className="btn primary" onClick={approve} disabled={busy === "approve"}>{busy === "approve" ? "מאשר..." : "אשר Ruleset"}</button>}
      </div>
    </div>
    {actionError && <p className="quiet">שגיאה: {actionError}</p>}

    <div className="grid-2">
      <section className="workspace-card">
        <h2>מקורות משפטיים</h2>
        {rs.sources.length === 0 && <p className="quiet">אין עדיין מקורות.</p>}
        {rs.sources.map(s => <div className="authority-row" key={s.id}>
          <div><strong>{s.citation}</strong><small>{s.pinpoint ?? ""}{s.documentVersionId ? " · מסמך בתיק" : " · ציטוט חיצוני"}</small></div>
          <StatusBadge tone={s.verifiedAt ? "ok" : "warn"}>{s.verifiedAt ? "מאומת" : "לא מאומת"}</StatusBadge>
        </div>)}
        {isDraft && <div style={{ marginTop: 12 }}>
          <label>סוג מקור<select value={sourceKind} onChange={e => setSourceKind(e.target.value)}>
            <option value="legislation">legislation</option><option value="regulation">regulation</option>
            <option value="judgment">judgment</option><option value="official_guidance">official_guidance</option>
            <option value="internal_legal_memo">internal_legal_memo</option>
          </select></label>
          <label>ציטוט<input value={citation} onChange={e => setCitation(e.target.value)} placeholder="חוק / תקנה / פסק דין" /></label>
          <label>מיקום מדויק (Pinpoint)<input value={pinpoint} onChange={e => setPinpoint(e.target.value)} /></label>
          <label>מאומת ע"י (לציטוט חיצוני בלבד)<input value={verifiedBy} onChange={e => setVerifiedBy(e.target.value)} placeholder="שם עורך הדין" /></label>
          <button className="btn secondary" onClick={addSource} disabled={busy === "source" || !citation.trim()}>הוסף מקור</button>
        </div>}
      </section>

      <section className="workspace-card">
        <h2>כללים</h2>
        {rs.rules.length === 0 && <p className="quiet">אין עדיין כללים.</p>}
        {rs.rules.map(r => <div className="fact-row" key={r.id}>
          <strong>{r.ruleKey}</strong> <small>עדיפות {r.priority}</small>
          <p className="quiet" style={{ fontFamily: "monospace", fontSize: 11 }}>תנאים: {JSON.stringify(r.conditions)}</p>
          <p className="quiet" style={{ fontFamily: "monospace", fontSize: 11 }}>פעולות: {JSON.stringify(r.operation)}</p>
        </div>)}
        {isDraft && <div style={{ marginTop: 12 }}>
          <label>מפתח כלל<input value={ruleKey} onChange={e => setRuleKey(e.target.value)} /></label>
          <label>עדיפות<input value={priority} onChange={e => setPriority(e.target.value)} /></label>
          <label>תנאים (JSON)<textarea rows={3} value={conditionsText} onChange={e => setConditionsText(e.target.value)} /></label>
          <label>פעולות (JSON)<textarea rows={3} value={operationText} onChange={e => setOperationText(e.target.value)} /></label>
          <label>תבנית הסבר<input value={explanationTemplate} onChange={e => setExplanationTemplate(e.target.value)} placeholder="המועד נקבע ל-{result}" /></label>
          <label>מקור<select value={ruleSourceId} onChange={e => setRuleSourceId(e.target.value)}>
            <option value="">ללא</option>
            {rs.sources.map(s => <option key={s.id} value={s.id}>{s.citation}</option>)}
          </select></label>
          <button className="btn secondary" onClick={addRule} disabled={busy === "rule" || !ruleKey.trim()}>הוסף כלל</button>
        </div>}
      </section>
    </div>

    <section className="workspace-card">
      <div className="card-head"><h2>מקרי בדיקה</h2>
        <button className="btn secondary" onClick={runTests} disabled={busy === "run"}>{busy === "run" ? "מריץ..." : "הרץ בדיקות"}</button></div>
      {rs.testCases.length === 0 && <p className="quiet">אין עדיין מקרי בדיקה.</p>}
      {rs.testCases.map(tc => {
        const result = testResults?.find(r => r.name === tc.name);
        return <div className="fact-row" key={tc.id}>
          <strong>{tc.name}</strong>
          <p className="quiet" style={{ fontFamily: "monospace", fontSize: 11 }}>קלט: {JSON.stringify(tc.input)}</p>
          <p className="quiet" style={{ fontFamily: "monospace", fontSize: 11 }}>פלט צפוי: {JSON.stringify(tc.expectedOutput)}</p>
          <div className="proposal-actions">
            <StatusBadge tone={tc.reviewStatus === "approved" ? "ok" : tc.reviewStatus === "rejected" ? "risk" : "neutral"}>{tc.reviewStatus}</StatusBadge>
            {result && <StatusBadge tone={result.passed ? "ok" : "risk"}>{result.passed ? "עבר" : "נכשל"}: {result.detail}</StatusBadge>}
            {isDraft && tc.reviewStatus !== "approved" &&
              <button disabled={busy === tc.id} onClick={() => reviewTestCase(tc.id, true)}>אשר מקרה בדיקה</button>}
          </div>
        </div>;
      })}
      {isDraft && <div style={{ marginTop: 12 }}>
        <label>שם<input value={tcName} onChange={e => setTcName(e.target.value)} /></label>
        <label>קלט (JSON)<textarea rows={2} value={tcInput} onChange={e => setTcInput(e.target.value)} /></label>
        <label>פלט צפוי (JSON)<textarea rows={2} value={tcExpected} onChange={e => setTcExpected(e.target.value)} /></label>
        <button className="btn secondary" onClick={addTestCase} disabled={busy === "tc" || !tcName.trim()}>הוסף מקרה בדיקה</button>
      </div>}
    </section>

    {rs.status === "approved" && <section className="workspace-card">
      <h2>תצוגה מקדימה של הפעלת המנוע</h2>
      <p className="quiet">בדיקה בלבד — לא נשמר דבר. לשיוך תוצאה לתיק ולנעילתה יש להשתמש בפקודת commit_legal_engine_run מהתיק הרלוונטי.</p>
      <label>הקשר קלט (JSON)<textarea rows={3} value={previewContext} onChange={e => setPreviewContext(e.target.value)} /></label>
      <button className="btn secondary" onClick={runPreview} disabled={busy === "preview"}>{busy === "preview" ? "מריץ..." : "תצוגה מקדימה"}</button>
      {previewResult && <div className="legal-note"><strong>{previewResult.matchedRuleKey}</strong><p>{previewResult.explanation}</p></div>}
    </section>}
  </div>;
}

export function LegalRulesPage({ onBack }: { onBack: () => void }) {
  const [openId, setOpenId] = useState<string | null>(null);
  return openId
    ? <RulesetEditor rulesetId={openId} onBack={() => setOpenId(null)} />
    : <>
      <button className="back-link" onClick={onBack}>‹ חזרה להגדרות</button>
      <RulesetList onOpen={setOpenId} />
    </>;
}
