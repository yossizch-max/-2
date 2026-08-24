import type {
  ActionItem, Authority, DamageCalculation, Deadline, DocumentRow,
  LegalDocument, Matter, Task, VerifiedFact
} from "../types";

export const matters: Matter[] = [
  {
    id: "m1", title: "כהן נ׳ כלל חברה לביטוח", internalNumber: "184/26",
    matterType: "personal_injury", status: "active",
    workflowStage: "treatment_and_records", folderPath: "C:\\Office\\184-26 כהן",
    documentCount: 42, verifiedFactCount: 12, pendingReviewCount: 3,
    updatedAt: "2026-08-24T07:02:00+03:00"
  },
  {
    id: "m2", title: "עזבון אבו סאלח", internalNumber: "211/26",
    matterType: "wrongful_death", status: "active",
    workflowStage: "evidence_collection", folderPath: "C:\\Office\\211-26 אבו סאלח",
    documentCount: 67, verifiedFactCount: 8, pendingReviewCount: 4,
    updatedAt: "2026-08-23T18:10:00+03:00"
  }
];

export const documents: DocumentRow[] = [
  { id:"d1", matterId:"m1", fileName:"סיכום אשפוז הדסה.pdf", category:"medical", sourceState:"local", extractionState:"complete", currentVersionId:"v1", currentSha256:"demo1", modifiedAt:"2026-08-23T12:00:00+03:00" },
  { id:"d2", matterId:"m1", fileName:"החלטה 20.8.2026.pdf", category:"court", sourceState:"local", extractionState:"complete", currentVersionId:"v2", currentSha256:"demo2", modifiedAt:"2026-08-20T16:20:00+03:00" },
  { id:"d3", matterId:"m1", fileName:"תלושי שכר 2026.pdf", category:"wage", sourceState:"cloud_only", extractionState:"blocked", modifiedAt:"2026-08-18T09:00:00+03:00" }
];

export const tasks: Task[] = [
  { id:"t1", matterId:"m1", title:"מעקב אחר רשומות הדסה", dueAt:"2026-08-28", status:"open", riskClass:"one_click" },
  { id:"t2", matterId:"m1", title:"בדיקת החלטה חדשה", dueAt:"2026-08-25", status:"open", riskClass:"approval_required" }
];

export const deadlines: Deadline[] = [
  { id:"dl1", matterId:"m1", action:"הגשת תגובה", dueAt:"2026-08-27", state:"committed", sourceLabel:"החלטה 20.8.2026, עמ׳ 2", ruleLabel:"מועד מפורש בהחלטה" }
];

export const facts: VerifiedFact[] = [
  { id:"f1", matterId:"m1", subject:"התובע", predicate:"אושפז", value:"הדסה עין כרם, 3 ימים", sourceLabel:"סיכום אשפוז, עמ׳ 1", stale:false, verifiedAt:"2026-08-23T14:00:00+03:00" },
  { id:"f2", matterId:"m1", subject:"אירוע", predicate:"תאריך", value:"14.04.2026", sourceLabel:"כתב תביעה, עמ׳ 2", stale:false, verifiedAt:"2026-08-23T14:10:00+03:00" }
];

export const calculations: DamageCalculation[] = [
  { id:"c1", matterId:"m1", regime:"pip", lifeState:"living", status:"locked", grossCents:13850000, deductionsCents:1850000, netCents:12000000, integritySha256:"demo-sha" }
];

export const legalDocuments: LegalDocument[] = [
  { id:"ld1", matterId:"m1", kind:"demand", title:"טיוטת דרישה", status:"draft", currentVersionId:"ldv1", updatedAt:"2026-08-24T08:00:00+03:00" }
];

export const authorities: Authority[] = [
  { id:"a1", matterId:"m1", citation:"רע״א 1234/20", title:"פלוני נ׳ חברת ביטוח", status:"verified" }
];

export const today: ActionItem[] = [
  { id:"a1", matterId:"m1", matterTitle:"כהן נ׳ כלל", kind:"critical", title:"מועד הגשת תגובה 27.08.2026", subtitle:"מועד מחייב, מקור אומת", actionLabel:"פתח מועד" },
  { id:"a2", matterId:"m1", matterTitle:"כהן נ׳ כלל", kind:"review", title:"3 הצעות עובדה ממתינות", subtitle:"AI אינו מאשר עובדות", actionLabel:"בדוק" },
  { id:"a3", matterId:"m2", matterTitle:"עזבון אבו סאלח", kind:"waiting", title:"ממתין למסמכים רפואיים", subtitle:"Follow-up בעוד 4 ימים", actionLabel:"פתח" }
];
