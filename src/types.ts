export type Matter = {
  id: string;
  title: string;
  internalNumber?: string | null;
  externalNumber?: string | null;
  matterType: string;
  status: "active" | "closed" | "archived";
  workflowStage: string;
  folderPath?: string | null;
  documentCount: number;
  verifiedFactCount: number;
  pendingReviewCount: number;
  updatedAt: string;
};

export type DocumentRow = {
  id: string;
  matterId: string;
  fileName: string;
  category: string;
  sourceState: "local" | "cloud_only" | "unknown";
  extractionState: "not_started" | "pending" | "complete" | "blocked" | "stale";
  currentVersionId?: string | null;
  currentSha256?: string | null;
  modifiedAt: string;
  occurrenceId?: string | null;
};

export type ActionItem = {
  id: string;
  matterId?: string;
  matterTitle?: string;
  kind: "critical" | "review" | "waiting" | "resume" | "new";
  title: string;
  subtitle: string;
  actionLabel: string;
};

export type Deadline = {
  id: string; matterId: string; action: string; dueAt: string;
  state: "draft" | "committed" | "superseded" | "completed";
  sourceLabel: string; ruleLabel?: string | null;
};

export type Task = {
  id: string; matterId: string; title: string; dueAt?: string | null;
  status: "open" | "done" | "cancelled";
  riskClass: "safe_auto" | "one_click" | "approval_required";
};

export type VerifiedFact = {
  id: string; matterId: string; subject: string; predicate: string; value: string;
  sourceLabel: string; stale: boolean; verifiedAt: string;
};

export type DamageCalculation = {
  id: string; matterId: string; regime: "pip" | "tort";
  lifeState: "living" | "death"; status: "draft" | "locked";
  grossCents: number; deductionsCents: number; netCents: number;
  integritySha256?: string | null;
  inputs?: Array<{key: string; cents: number; source: string}>;
};

export type LegalDocument = {
  id: string; matterId: string; kind: string; title: string;
  status: "draft" | "approved" | "superseded";
  currentVersionId?: string | null; updatedAt: string;
};

export type LegalDocumentParagraph = {
  id: string; index: number; kind: string; bodyText: string;
  provenanceState: "confirmed" | "needs_review";
};

export type LegalDocumentSection = {
  id: string; index: number; heading: string; paragraphs: LegalDocumentParagraph[];
};

export type LegalDocumentVersionDetail = {
  id: string; legalDocumentId: string; versionNumber: number;
  status: "draft" | "approved" | "superseded"; sections: LegalDocumentSection[];
};

export type Authority = {
  id: string; matterId: string; citation: string; title: string;
  status: "draft" | "verified" | "revoked";
};
