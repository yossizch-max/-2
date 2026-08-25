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
  dueAt?: string | null;
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
  sourceLabel: string; stale: boolean; verifiedAt: string; occurrenceId?: string | null;
};

export type AiProfile = {
  id: string; providerKind: "local" | "openai"; baseUrl: string;
  model?: string | null; enabled: boolean; clientDataAuthorized: boolean;
};

export type SourceExcerpt = {
  sourceId: string; page?: number | null; fileName?: string | null;
  excerpt: string; truncated: boolean;
};

export type AiProposal = {
  id: string; proposalKind: string;
  structured: {subject?: string; predicate?: string; value?: string; sourceIds?: string[]};
  status: "pending" | "approved" | "rejected" | "needs_revision";
  reviewedAt?: string | null; reviewNote?: string | null;
  sourceExcerpts: SourceExcerpt[];
};

export type AiRun = {
  id: string; matterId?: string | null; capability: string; status: string;
  model?: string | null; startedAt: string; finishedAt?: string | null;
  proposals: AiProposal[];
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

export type LegalRuleset = {
  id: string; engineKind: string; jurisdiction: string; title: string; version: string;
  status: "draft" | "under_review" | "approved" | "superseded" | "revoked";
  effectiveFrom?: string | null; effectiveTo?: string | null;
  approvedAt?: string | null; approvedBy?: string | null; supersededBy?: string | null;
  sourceCount: number; testCaseCount: number; approvedTestCaseCount: number;
};

export type LegalRulesetSource = {
  id: string; sourceKind: string; citation: string; pinpoint?: string | null;
  documentVersionId?: string | null; documentPageId?: string | null;
  sourceSha256: string; verifiedAt?: string | null; verifiedBy?: string | null;
};

export type LegalRule = {
  id: string; ruleKey: string; ruleType: string; priority: number;
  conditions: unknown; operation: unknown;
  explanationTemplate?: string | null; sourceId?: string | null;
};

export type LegalRuleTestCase = {
  id: string; name: string; input: unknown; expectedOutput: unknown;
  reviewStatus: "draft" | "approved" | "rejected";
  reviewedBy?: string | null; reviewedAt?: string | null;
};

export type LegalRulesetDetail = LegalRuleset & {
  description?: string | null; createdAt: string; createdBy?: string | null;
  submittedForReviewAt?: string | null; integritySha256?: string | null;
  sources: LegalRulesetSource[]; rules: LegalRule[]; testCases: LegalRuleTestCase[];
};

export type Authority = {
  id: string; matterId: string; citation: string; title: string;
  status: "draft" | "verified" | "revoked";
  sourceDocumentVersionId?: string | null; approvedPassageCount: number;
};

export type AuthorityPassage = {
  id: string; sourcePageId: string; passageText: string; issueTag?: string | null;
  approved: boolean; page?: number | null; fileName?: string | null;
};

export type DocumentPage = {
  id: string; pageNumber?: number | null; anchorKind: string; blockIndex: number;
  text: string; textSha256: string; method: string;
};
