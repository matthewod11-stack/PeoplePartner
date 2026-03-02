// HR Command Center - Type Definitions
// These types mirror the SQLite schema and API responses

// =============================================================================
// Database Models
// =============================================================================

export interface Employee {
  id: string;
  email: string;
  full_name: string;
  department?: string;
  job_title?: string;
  manager_id?: string;
  hire_date?: string;
  work_state?: string;
  status: 'active' | 'terminated' | 'leave';

  // Demographics (V1 expansion)
  date_of_birth?: string;
  gender?: string;
  ethnicity?: string;

  // Termination details
  termination_date?: string;
  termination_reason?: 'voluntary' | 'involuntary' | 'retirement' | 'other';

  extra_fields?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface Conversation {
  id: string;
  title?: string;
  summary?: string;
  messages: Message[];
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
  error?: ChatError;
  /** V2.1.4: Answer verification result for aggregate queries */
  verification?: VerificationResult;
}

// =============================================================================
// Chat Error Types (Phase 3.5)
// =============================================================================

export type ChatErrorType =
  | 'no_api_key'
  | 'trial_limit'
  | 'auth_error'
  | 'rate_limit'
  | 'network_error'
  | 'api_error'
  | 'unknown';

export interface ChatError {
  type: ChatErrorType;
  message: string;
  details: string;
  retryable: boolean;
  originalContent?: string;
}

// =============================================================================
// Answer Verification Types (V2.1.4)
// =============================================================================

export type VerificationStatus =
  | 'Verified'
  | 'PartialMatch'
  | 'Unverified'
  | 'NotApplicable';

export type ClaimType =
  | 'TotalHeadcount'
  | 'ActiveCount'
  | 'DepartmentCount'
  | 'AvgRating'
  | 'EnpsScore'
  | 'TurnoverRate'
  | 'Percentage';

export interface NumericClaim {
  claim_type: ClaimType;
  value_found: number;
  ground_truth: number | null;
  is_match: boolean;
}

export interface VerificationResult {
  is_aggregate_query: boolean;
  claims: NumericClaim[];
  overall_status: VerificationStatus;
  sql_query: string | null;
}

/** Query classification for context-adaptive retrieval */
export type QueryType =
  | 'Aggregate'
  | 'List'
  | 'Individual'
  | 'Comparison'
  | 'Attrition'
  | 'General';

/** Organization aggregates for verification ground truth */
export interface OrgAggregates {
  total_employees: number;
  active_count: number;
  terminated_count: number;
  on_leave_count: number;
  by_department: DepartmentCount[];
  avg_rating: number | null;
  rating_distribution: RatingDistribution;
  employees_with_no_rating: number;
  enps: EnpsAggregate;
  attrition: AttritionStats;
}

export interface DepartmentCount {
  name: string;
  count: number;
  percentage: number;
}

export interface RatingDistribution {
  exceptional: number;
  exceeds: number;
  meets: number;
  needs_improvement: number;
}

export interface EnpsAggregate {
  score: number;
  promoters: number;
  passives: number;
  detractors: number;
  total_responses: number;
}

export interface AttritionStats {
  terminations_ytd: number;
  voluntary: number;
  involuntary: number;
  avg_tenure_months: number | null;
  turnover_rate_annualized: number | null;
}

/** Result from get_system_prompt command (V2.1.4, V2.2.2) */
export interface SystemPromptResult {
  system_prompt: string;
  employee_ids_used: string[];
  aggregates: OrgAggregates | null;
  query_type: QueryType;
  metrics: RetrievalMetrics; // V2.2.2: retrieval observability
}

// =============================================================================
// Token Budget & Retrieval Metrics (V2.2.2)
// =============================================================================

/** Token budget allocation per query type */
export interface TokenBudget {
  /** Token budget for employee context (profiles or summaries) */
  employee_context: number;
  /** Token budget for theme context (future: V2.2.2b) */
  theme_context: number;
  /** Token budget for memory context (past conversations) */
  memory_context: number;
  /** Combined total budget */
  total_context: number;
}

/** Actual token usage tracked during context retrieval */
export interface TokenUsage {
  /** Tokens used by employee context */
  employee_tokens: number;
  /** Tokens used by memory context */
  memory_tokens: number;
  /** Tokens used by organization aggregates */
  aggregates_tokens: number;
  /** Total tokens used (sum of all sections) */
  total_tokens: number;
}

/** Comprehensive retrieval metrics for observability */
export interface RetrievalMetrics {
  /** Query type classification */
  query_type: QueryType;
  /** Number of employees matched by query */
  employees_found: number;
  /** Number of employees included in context */
  employees_included: number;
  /** Number of memories matched by query */
  memories_found: number;
  /** Number of memories included in context */
  memories_included: number;
  /** Whether organization aggregates were included */
  aggregates_included: boolean;
  /** Token budget allocation for this query type */
  token_budget: TokenBudget;
  /** Actual token usage */
  token_usage: TokenUsage;
  /** Total retrieval time in milliseconds */
  retrieval_time_ms: number;
}

export interface Company {
  id: string;
  name: string;
  state: string; // HQ/incorporation state (2-letter code)
  industry?: string;
  created_at: string;
}

/** Input for creating or updating the company profile */
export interface UpsertCompany {
  name: string;
  state: string; // 2-letter US state code (e.g., CA, NY, TX)
  industry?: string;
}

/** Count of employees in a specific state */
export interface StateCount {
  state: string;
  count: number;
}

/** Summary of states where employees work (operational footprint) */
export interface EmployeeStatesSummary {
  /** List of state codes with employees */
  states: string[];
  /** Breakdown by state with counts */
  counts: StateCount[];
}

export interface Settings {
  key: string;
  value: string;
  updated_at: string;
}

export interface AuditLogEntry {
  id: string;
  conversation_id?: string;
  request_redacted: string;
  response_text: string;
  context_used?: string[];
  created_at: string;
}

// =============================================================================
// Performance & Engagement (V1 Expansion)
// =============================================================================

export interface ReviewCycle {
  id: string;
  name: string;
  cycle_type: 'annual' | 'semi-annual' | 'quarterly';
  start_date: string;
  end_date: string;
  status: 'active' | 'closed';
  created_at: string;
}

export interface PerformanceRating {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  overall_rating: number;        // 1.0 - 5.0
  goals_rating?: number;
  competencies_rating?: number;
  reviewer_id?: string;
  rating_date?: string;
  created_at: string;
  updated_at: string;
}

export interface PerformanceReview {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  strengths?: string;
  areas_for_improvement?: string;
  accomplishments?: string;
  goals_next_period?: string;
  manager_comments?: string;
  self_assessment?: string;
  reviewer_id?: string;
  review_date?: string;
  created_at: string;
  updated_at: string;
}

export interface EnpsResponse {
  id: string;
  employee_id: string;
  score: number;                 // 0-10
  survey_date: string;
  survey_name?: string;
  feedback_text?: string;
  created_at: string;
}

// Computed types for analytics
export type EnpsCategory = 'promoter' | 'passive' | 'detractor';

export interface EmployeeWithPerformance extends Employee {
  latest_rating?: number;
  latest_enps?: number;
  enps_category?: EnpsCategory;
}

// Rating scale reference (for UI display)
export const RATING_LABELS: Record<number, string> = {
  5: 'Exceptional',
  4: 'Exceeds',
  3: 'Meets',
  2: 'Developing',
  1: 'Unsatisfactory',
};

// eNPS category helper
export function getEnpsCategory(score: number): EnpsCategory {
  if (score >= 9) return 'promoter';
  if (score >= 7) return 'passive';
  return 'detractor';
}

// =============================================================================
// AI Provider (Phase E)
// =============================================================================

/** AI provider info returned from the backend */
export interface ProviderInfo {
  id: string;
  display_name: string;
  key_prefix_hint: string;
}

// =============================================================================
// Application State
// =============================================================================

export interface AppState {
  isOnline: boolean;
  hasApiKey: boolean;
  hasCompanyProfile: boolean;
  isOnboarded: boolean;
}

// =============================================================================
// PII Redaction
// =============================================================================

export type PIIType = 'SSN' | 'CreditCard' | 'BankAccount';

export interface PIIRedaction {
  type: PIIType;
  original: string;
  placeholder: string;
}

// =============================================================================
// API Responses
// =============================================================================

export interface ChatResponse {
  message: string;
  redactions?: PIIRedaction[];
}

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

// =============================================================================
// File Parsing (Phase 2.1.B)
// =============================================================================

/**
 * A single parsed row from a file (column_name -> value)
 */
export type ParsedRow = Record<string, string>;

/**
 * Result of fully parsing a file
 */
export interface ParseResult {
  /** Column headers from the first row */
  headers: string[];
  /** All data rows (excluding header) */
  rows: ParsedRow[];
  /** Total number of data rows */
  total_rows: number;
  /** Detected file format (CSV, TSV, XLSX, XLS) */
  file_format: string;
  /** Warnings during parsing (e.g., skipped rows) */
  warnings: string[];
}

/**
 * Preview result (limited rows for UI display)
 */
export interface ParsePreview {
  /** Column headers */
  headers: string[];
  /** First N rows for preview */
  preview_rows: ParsedRow[];
  /** Total rows in file (not just preview) */
  total_rows: number;
  /** Detected file format */
  file_format: string;
}

/**
 * Mapping of standard field name to parsed header name
 */
export type ColumnMapping = Record<string, string>;

/** Supported file extensions for import */
export const SUPPORTED_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls'] as const;
export type SupportedExtension = (typeof SUPPORTED_EXTENSIONS)[number];

// =============================================================================
// Review Highlights (V2.2.1)
// =============================================================================

/**
 * A quote extracted from a performance review
 */
export interface ReviewQuote {
  sentiment: 'positive' | 'negative' | 'neutral';
  text: string;
}

/**
 * Extracted highlights from a single performance review
 */
export interface ReviewHighlight {
  id: string;
  review_id: string;
  employee_id: string;
  review_cycle_id: string;
  strengths: string[];
  opportunities: string[];
  themes: string[];
  quotes: ReviewQuote[];
  overall_sentiment: 'positive' | 'neutral' | 'mixed' | 'negative';
  extraction_model?: string;
  extraction_version: number;
  token_count?: number;
  created_at: string;
  updated_at: string;
}

/**
 * Aggregated career summary for an employee
 */
export interface EmployeeSummary {
  id: string;
  employee_id: string;
  career_narrative?: string;
  key_strengths: string[];
  development_areas: string[];
  notable_accomplishments: string[];
  reviews_analyzed: number;
  last_review_date?: string;
  generation_model?: string;
  created_at: string;
  updated_at: string;
}

/**
 * Result of batch extraction operation
 */
export interface BatchExtractionResult {
  total: number;
  succeeded: number;
  failed: number;
  errors: string[];
}

/**
 * Valid theme values for review highlights
 */
export const VALID_THEMES = [
  'leadership',
  'technical-growth',
  'communication',
  'collaboration',
  'execution',
  'learning',
  'innovation',
  'mentoring',
  'problem-solving',
  'customer-focus',
] as const;
export type ReviewTheme = (typeof VALID_THEMES)[number];

// =============================================================================
// V2.5.1 - Data Quality Center Types
// =============================================================================

/** Import data type identifier */
export type ImportType = 'employees' | 'ratings' | 'reviews' | 'enps';

/** Result of normalizing a source header */
export interface HeaderNormalization {
  /** Original header text from the file */
  original: string;
  /** Normalized version (trimmed, lowercased, underscored) */
  normalized: string;
  /** Auto-detected target field (null if no match) */
  detectedField: string | null;
  /** Confidence score of auto-detection (0.0 - 1.0) */
  confidence: number;
}

/** Known HRIS systems with preset column mappings */
export type HrisPresetId = 'workday' | 'bamboohr' | 'gusto' | 'adp' | 'rippling' | 'generic';

/** An HRIS preset definition */
export interface HrisPreset {
  id: HrisPresetId;
  name: string;
  description: string;
  /** Mapping of target field -> expected source header(s) */
  mappings: Record<string, string[]>;
}

/** A single validation issue for a cell */
export interface ValidationIssue {
  /** 1-based row number in the source file */
  row: number;
  /** Column/field that has the issue */
  column: string;
  /** The problematic value */
  value: string;
  /** Human-readable error message */
  message: string;
  /** Severity level */
  severity: 'error' | 'warning';
  /** Error type classification */
  errorType: 'missing_required' | 'invalid_format' | 'out_of_range' | 'invalid_email'
    | 'invalid_date' | 'invalid_state' | 'unknown_value' | 'duplicate_in_file';
}

/** Result of validating import data */
export interface ImportValidationResult {
  /** Whether all rows are valid */
  isValid: boolean;
  /** List of all issues found */
  issues: ValidationIssue[];
  /** Count of rows with errors */
  errorRowCount: number;
  /** Count of rows with warnings only */
  warningRowCount: number;
  /** Count of completely clean rows */
  cleanRowCount: number;
}

/** A row that has validation issues, with references to its issues */
export interface IssueRow {
  /** Index in the original data array */
  rowIndex: number;
  /** The row data */
  data: ParsedRow;
  /** Issues for this row */
  issues: ValidationIssue[];
}

/** A group of potential duplicate records */
export interface DuplicateGroup {
  /** Unique ID for this group */
  id: string;
  /** The incoming row from the file */
  incoming: ParsedRow;
  /** The existing record in the database */
  existing: {
    id: string;
    email: string;
    full_name: string;
    department?: string;
    [key: string]: string | undefined;
  };
  /** Match reason (e.g. "email match", "name similarity") */
  matchReason: string;
  /** Match confidence (0.0 - 1.0) */
  confidence: number;
}

/** User's resolution for a duplicate */
export interface DuplicateResolution {
  /** Group ID */
  groupId: string;
  /** Resolution action */
  action: 'keep_new' | 'keep_existing' | 'skip';
}

// ============================================================================
// Document Ingestion (V3.0)
// ============================================================================

/** Stats for the configured document folder */
export interface DocumentFolderStats {
  path: string;
  label: string | null;
  file_count: number;
  chunk_count: number;
  pii_file_count: number;
  error_file_count: number;
  last_scanned_at: string | null;
}

/** Aggregate indexing stats for the active document folder */
export interface DocumentStats {
  total_files: number;
  total_chunks: number;
  files_with_pii: number;
  files_with_errors: number;
  files_by_type: Record<string, number>;
  last_scanned_at: string | null;
}
