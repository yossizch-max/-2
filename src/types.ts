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
  extractionState: "not_started" | "pending" | "complete" | "blocked" | "stale" | "failed";
  currentVersionId?: string | null;
  currentSha256?: string | null;
  modifiedAt: string;
  occurrenceId?: string | null;
  categorySource?: "auto" | "manual" | null;
  categoryConfidence?: number | null;
  pageCount: number;
  extractionMethod?: string | null;
  lastErrorCode?: string | null;
};

export type DocumentIntakeOutcome = {
  documentId: string;
  fileName: string;
  outcome: "already_complete" | "extracted" | "ocred" | "classified" | "unsupported" | "failed";
  category?: string | null;
  errorCode?: string | null;
  errorMessage?: string | null;
};

export type DocumentIntakeSummary = {
  discovered: number;
  hashed: number;
  alreadyComplete: number;
  extracted: number;
  ocred: number;
  classified: number;
  failed: number;
  unsupported: number;
  documents: DocumentIntakeOutcome[];
};

export type ExtractionRun = {
  id: string;
  matterId: string;
  documentVersionId: string;
  sourceSha256: string;
  status: "running" | "completed" | "failed";
  errorCode?: string | null;
  startedAt: string;
  finishedAt?: string | null;
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

export type AiStructuredProposal = {
  sourceIds?: string[];
  subject?: string;
  predicate?: string;
  value?: string;
  eventDate?: string | null;
  providerName?: string | null;
  treatmentSummary?: string;
  periodStart?: string | null;
  periodEnd?: string | null;
  employerName?: string | null;
  grossAmountCents?: number | null;
  claimBasis?: string | null;
  liablePartyName?: string | null;
  description?: string;
  explanation?: string | null;
  // Phase C, milestone C2: Matter Understanding item fields
  entityType?: string;
  displayName?: string;
  eventType?: string;
  title?: string;
  datePrecision?: string | null;
  documentDate?: string | null;
  involvedEntities?: string[];
  assertedBy?: string;
  statement?: string;
  target?: string | null;
  amountType?: string;
  amountCents?: number;
  currency?: string;
  context?: string | null;
  date?: string;
  dateType?: string;
  issueType?: string;
  itemA?: string;
  sourceAId?: string;
  itemB?: string;
  sourceBId?: string;
  reason?: string;
  question?: string;
  confidence?: number | null;
  // Phase C, milestone C3: Medical Evidence Intelligence item fields
  encounterType?: string;
  provider?: string | null;
  institution?: string | null;
  specialty?: string | null;
  complaint?: string;
  bodyRegion?: string | null;
  laterality?: string | null;
  severity?: string | null;
  duration?: string | null;
  finding?: string;
  measurement?: string | null;
  diagnosisText?: string;
  code?: string | null;
  certainty?: string;
  testType?: string;
  stage?: string;
  orderedDate?: string | null;
  performedDate?: string | null;
  resultDate?: string | null;
  interpretation?: string | null;
  treatmentType?: string;
  frequency?: string | null;
  outcome?: string | null;
  medication?: string;
  dosage?: string | null;
  route?: string | null;
  startDate?: string | null;
  endDate?: string | null;
  status?: string;
  planType?: string;
  urgency?: string | null;
  limitation?: string;
  workCapacityStatus?: string;
  determiningBody?: string;
  disabilityType?: string | null;
  percentage?: number | null;
  durationType?: string;
  regulation?: string | null;
  opinionType?: string;
  opinionText?: string;
  author?: string | null;
  bodyRegionOrStream?: string | null;
  priorEncounterRef?: string | null;
  nextEncounterRef?: string | null;
  signalReason?: string;
  missingType?: string;
  // Phase C, milestone C4: Wage/Economic + Liability Evidence Intelligence item fields
  employer?: string;
  role?: string | null;
  employmentStatus?: string;
  amountBasis?: string;
  incomeType?: string;
  employerOrSource?: string | null;
  month?: string;
  netAmountCents?: number | null;
  components?: string | null;
  sourceType?: string;
  year?: string;
  statedReason?: string | null;
  documentedBy?: string | null;
  issuingSource?: string;
  changeType?: string;
  paymentType?: string;
  payer?: string | null;
  gapType?: string;
  witness?: string;
  issue?: string | null;
  evidenceType?: string;
  reportType?: string;
  factualContent?: string;
  vehicle?: string | null;
  damageLocation?: string | null;
  documentedCondition?: string;
  mediaType?: string | null;
  expert?: string;
  position?: string;
  detail?: string | null;
  insurer?: string | null;
  findingType?: string;
  court?: string | null;
  // Phase C, milestone C4 v2: regime-aware expansion - additional item fields
  periodPrecision?: string | null;
  overtimeCents?: number | null;
  bonusCents?: number | null;
  pensionContributionCents?: number | null;
  monthsWorked?: number | null;
  jobDescription?: string | null;
  hoursText?: string | null;
  statedSalaryText?: string | null;
  terminationReasonStated?: string | null;
  documentType?: string;
  taxYear?: string;
  revenueCents?: number | null;
  expensesCents?: number | null;
  profitCents?: number | null;
  employerContributionCents?: number | null;
  employeeContributionCents?: number | null;
  pensionComponent?: string | null;
  trainingFund?: string | null;
  unitsText?: string | null;
  incapacityDegreeText?: string | null;
};

// Phase C, milestone C2: Matter Understanding Core
export type TimelineItem = {
  id: string; kind: string; businessDate: string; title: string;
  description?: string | null; verified: boolean; insertedAt: string;
  datePrecision?: string | null;
};

export type MatterBriefItem = { id: string; status: string; pending: boolean; structured: AiStructuredProposal };

export type MatterBrief = {
  matterId: string;
  profile: MatterProfile;
  parties: MatterParty[];
  entities: MatterBriefItem[];
  chronology: TimelineItem[];
  claims: MatterBriefItem[];
  amounts: MatterBriefItem[];
  issues: MatterBriefItem[];
  contradictions: MatterBriefItem[];
  missingInformation: MatterBriefItem[];
  verifiedFactCount: number;
  openConflictCount: number;
  pendingReviewCount: number;
};

// Phase C, milestone C3: Medical Evidence Intelligence
export type MedicalTimelineItem = {
  id: string; kind: string; businessDate?: string | null; datePrecision?: string | null;
  title: string; description?: string | null; verified: boolean; insertedAt: string;
};

export type PriorVsPostIncidentView = {
  incidentDate?: string | null;
  documentedBefore: MedicalTimelineItem[];
  documentedAfter: MedicalTimelineItem[];
  undated: MedicalTimelineItem[];
};

export type MedicalBriefItem = { id: string; status: string; pending: boolean; structured: AiStructuredProposal };

export type MedicalBrief = {
  matterId: string;
  mainTreatmentHistory: MedicalBriefItem[];
  keyComplaints: MedicalBriefItem[];
  objectiveFindings: MedicalBriefItem[];
  diagnoses: MedicalBriefItem[];
  testsImaging: MedicalBriefItem[];
  treatments: MedicalBriefItem[];
  functionalWorkLimitations: MedicalBriefItem[];
  disabilityDeterminations: MedicalBriefItem[];
  priorDocumentedHistory: MedicalBriefItem[];
  medicalOpinions: MedicalBriefItem[];
  candidateGaps: MedicalBriefItem[];
  missingEvidenceSignals: MedicalBriefItem[];
  contradictions: MedicalBriefItem[];
  chronology: MedicalTimelineItem[];
  pendingMedicalReviewCount: number;
};

// Phase C, milestone C4, Part A: Wage/Economic Evidence Intelligence
export type WageTimelineItem = {
  id: string; kind: string; businessDate?: string | null;
  title: string; description?: string | null; verified: boolean; insertedAt: string;
};

export type WageComparisonView = {
  incidentDate?: string | null;
  documentedBefore: WageTimelineItem[];
  documentedAfter: WageTimelineItem[];
  undated: WageTimelineItem[];
};

export type WageBriefItem = { id: string; status: string; pending: boolean; structured: AiStructuredProposal };

export type WageBrief = {
  matterId: string;
  employment: WageBriefItem[];
  income: WageBriefItem[];
  payslips: WageBriefItem[];
  annualIncome: WageBriefItem[];
  employerConfirmations: WageBriefItem[];
  selfEmployedIncome: WageBriefItem[];
  pensionContributions: WageBriefItem[];
  absences: WageBriefItem[];
  sickLeave: WageBriefItem[];
  workLimitations: WageBriefItem[];
  employmentChanges: WageBriefItem[];
  benefitPayments: WageBriefItem[];
  missingEvidenceSignals: WageBriefItem[];
  chronology: WageTimelineItem[];
  pendingWageReviewCount: number;
};

// Phase C, milestone C4, Part B: Liability Evidence Intelligence
export type LiabilityBriefItem = { id: string; status: string; pending: boolean; structured: AiStructuredProposal };

// The liability regime a matter's evidence is organized under - reused from
// `matters.matter_type` via `liability::liability_regime_for_matter`, never a
// separate classification model. `ftl_road_accident` = Israel's Compensation for
// Road Accident Victims Law (largely fault-independent); `ordinary_negligence` =
// general tort principles; `unknown_requires_review` = not yet determinable from
// the matter's own type, never guessed.
export type LiabilityRegime = "ftl_road_accident" | "ordinary_negligence" | "unknown_requires_review";

export type LiabilityBrief = {
  matterId: string;
  regime: LiabilityRegime;
  partyVersions: LiabilityBriefItem[];
  witnesses: LiabilityBriefItem[];
  sceneEvidence: LiabilityBriefItem[];
  policeEvidence: LiabilityBriefItem[];
  vehicleDamage: LiabilityBriefItem[];
  photoVideoEvidence: LiabilityBriefItem[];
  expertOpinions: LiabilityBriefItem[];
  admissions: LiabilityBriefItem[];
  insurerPositions: LiabilityBriefItem[];
  courtFindings: LiabilityBriefItem[];
  liabilityIssues: LiabilityBriefItem[];
  contradictions: LiabilityBriefItem[];
  pendingLiabilityReviewCount: number;
};

export type LiabilityMatrixRow = {
  issue: string | null;
  versions: LiabilityBriefItem[];
  witnesses: LiabilityBriefItem[];
  objectiveEvidence: LiabilityBriefItem[];
  unresolvedConflict: boolean;
};

export type LiabilityMatrix = { matterId: string; regime: LiabilityRegime; rows: LiabilityMatrixRow[] };

// Phase C, milestone C5: Action Orchestrator / Matter Agent Core. One backend
// source of truth for "what should the lawyer deal with next" - never an
// AI-computed score, never something the frontend re-ranks on its own.
export type ActionCandidate = {
  actionCode: string;
  matterId: string;
  matterTitle: string;
  title: string;
  reason: string;
  sourceType: string;
  sourceId?: string | null;
  targetId?: string | null;
  workstreamKind?: string | null;
  requirementKey?: string | null;
  dueAt?: string | null;
  urgency: string;
  blocking: boolean;
  rankCategory: number;
  humanActionOptions: string[];
  fingerprint: string;
  recommendationState: "active" | "acknowledged" | "snoozed" | "dismissed" | "converted_to_task";
  snoozedUntil?: string | null;
};

export type UrgencyCount = { urgency: string; count: number };

export type ActionPlan = {
  matterId: string;
  matterTitle: string;
  asOf: string;
  primaryAction: ActionCandidate | null;
  alternatives: ActionCandidate[];
  candidates: ActionCandidate[];
  blockers: string[];
  countsByUrgency: UrgencyCount[];
};

export type ActionCenterEntry = { matterId: string; matterTitle: string; plan: ActionPlan };

export type AiProposal = {
  id: string; proposalKind: string;
  structured: AiStructuredProposal;
  sourceManifestSha256?: string | null;
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

export const CASE_TYPES: Array<{value: string; label: string}> = [
  {value: "traffic_accident", label: "תאונת דרכים"},
  {value: "work_accident", label: "תאונת עבודה"},
  {value: "general_negligence", label: "רשלנות כללית"},
  {value: "medical_malpractice", label: "רשלנות רפואית"},
  {value: "civil_commercial", label: "סכסוך אזרחי/מסחרי"},
  {value: "generic_civil", label: "אזרחי כללי"},
  {value: "other", label: "אחר"},
];

export const PARTY_ROLES: Array<{value: string; label: string}> = [
  {value: "client", label: "לקוח"},
  {value: "party", label: "צד"},
  {value: "witness", label: "עד"},
  {value: "employer", label: "מעסיק"},
  {value: "insurer", label: "מבטחת"},
  {value: "medical_provider", label: "מוסד רפואי"},
  {value: "expert", label: "מומחה"},
  {value: "opposing_counsel", label: "עו\"ד צד שכנגד"},
  {value: "court", label: "בית משפט"},
];

export const ENTITY_KINDS: Array<{value: string; label: string}> = [
  {value: "unknown", label: "לא ידוע"},
  {value: "person", label: "אדם פרטי"},
  {value: "organization", label: "גוף/חברה"},
];

export type MatterProfile = {
  matterId: string; primaryEventDate?: string | null; primaryCourtName?: string | null;
  btlClaimNumber?: string | null; caseSummary?: string | null; updatedAt: string;
};

export type MatterParty = {
  id: string; matterId: string; role: string; displayName: string; entityKind: string;
  identifier?: string | null; phone?: string | null; email?: string | null; address?: string | null;
  notes?: string | null; createdAt: string; updatedAt: string;
};

export const WORKSTREAM_KINDS: Array<{value: string; label: string}> = [
  {value: "medical", label: "רפואי"},
  {value: "liability", label: "אחריות"},
  {value: "wage", label: "שכר"},
  {value: "insurance", label: "ביטוח"},
  {value: "btl", label: "מל\"ל"},
  {value: "negotiation", label: "מו\"מ"},
  {value: "litigation", label: "ליטיגציה"},
];

export const WORKSTREAM_STATUSES: Array<{value: string; label: string}> = [
  {value: "not_applicable", label: "לא רלוונטי"},
  {value: "not_started", label: "טרם התחיל"},
  {value: "active", label: "פעיל"},
  {value: "blocked", label: "חסום"},
  {value: "done", label: "הושלם"},
];

export type Workstream = {
  id: string; matterId: string; kind: string; status: string;
  notes?: string | null; createdAt: string; updatedAt: string;
};

export const REQUIREMENT_KEYS: Array<{value: string; label: string}> = [
  {value: "id_document", label: "תעודת זהות"},
  {value: "police_report", label: "דו\"ח משטרה"},
  {value: "medical_records_initial", label: "מסמכים רפואיים ראשוניים"},
  {value: "medical_records_full_file", label: "תיק רפואי מלא"},
  {value: "wage_stubs", label: "תלושי שכר"},
  {value: "employer_incident_report", label: "דיווח מעסיק על התאונה"},
  {value: "witness_statements", label: "הודעות עדים"},
  {value: "insurance_policy", label: "פוליסת ביטוח"},
  {value: "btl_forms", label: "טפסי מל\"ל"},
  {value: "vehicle_photos", label: "תמונות רכב/זירה"},
  {value: "expert_opinion", label: "חוות דעת מומחה"},
  {value: "contract_document", label: "מסמך חוזה"},
  {value: "correspondence_records", label: "תיעוד התכתבות"},
];

export const REQUIREMENT_STATUSES: Array<{value: string; label: string}> = [
  {value: "not_applicable", label: "לא רלוונטי"},
  {value: "not_collected", label: "טרם נאסף"},
  {value: "requested", label: "התבקש"},
  {value: "collected", label: "נאסף"},
  {value: "stale", label: "יש לרענן"},
];

export const REQUIREMENT_PRIORITIES: Array<{value: string; label: string}> = [
  {value: "recommended", label: "מומלץ"},
  {value: "required_by_office_policy", label: "נדרש לפי מדיניות המשרד"},
  {value: "optional", label: "אופציונלי"},
];

export type MatterRequirement = {
  id: string; matterId: string; requirementKey: string; status: string;
  relevance: string; priority?: string | null;
  notes?: string | null; createdAt: string; updatedAt: string;
};

export const LEDGER_STATUS_LABELS: Record<string, string> = {
  draft: "טיוטה",
  verified: "מאומת",
};

export type LedgerSource = {
  id: string; matterId: string; entryId: string; documentVersionId: string;
  documentPageId: string; displayQuote: string; sourceTextSha256: string;
};

export type MedicalEvent = {
  id: string; matterId: string; eventDate?: string | null; providerName?: string | null;
  treatmentSummary: string; status: string; stale: boolean; superseded: boolean;
  supersedesEntryId?: string | null; integritySha256?: string | null; verifiedAt?: string | null;
  createdAt: string; updatedAt: string;
};

export type WageRecord = {
  id: string; matterId: string; periodStart?: string | null; periodEnd?: string | null;
  employerName?: string | null; grossAmountCents: number;
  status: string; stale: boolean; superseded: boolean;
  supersedesEntryId?: string | null; integritySha256?: string | null; verifiedAt?: string | null;
  createdAt: string; updatedAt: string;
};

export type LiabilityFact = {
  id: string; matterId: string; claimBasis?: string | null; liablePartyName?: string | null;
  description: string; status: string; stale: boolean; superseded: boolean;
  supersedesEntryId?: string | null; integritySha256?: string | null; verifiedAt?: string | null;
  createdAt: string; updatedAt: string;
};
