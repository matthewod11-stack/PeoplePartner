// HR Command Center - Tauri Command Wrappers
// All Tauri invoke calls go through here for type safety

import { invoke } from '@tauri-apps/api/core';
import type {
  Employee,
  ReviewCycle,
  PerformanceRating,
  PerformanceReview,
  EnpsResponse,
  ParseResult,
  ParsePreview,
  ColumnMapping,
  Company,
  UpsertCompany,
  EmployeeStatesSummary,
  VerificationResult,
  OrgAggregates,
  QueryType,
  SystemPromptResult,
  // V2.2.1 - Review Highlights
  ReviewHighlight,
  EmployeeSummary,
  BatchExtractionResult,
  // Phase E - Provider Management
  ProviderInfo,
  // V3.0 - Document Ingestion
  DocumentFolderStats,
  DocumentStats,
} from './types';

/**
 * Test command - will be replaced with actual commands in Phase 1.4
 */
export async function greet(name: string): Promise<string> {
  return invoke('greet', { name });
}

// =============================================================================
// Phase 1.4 - API Key Management
// =============================================================================

/**
 * Store the Anthropic API key in macOS Keychain
 * @throws Error if key format is invalid or Keychain access fails
 */
export async function storeApiKey(apiKey: string): Promise<void> {
  return invoke('store_api_key', { apiKey });
}

/**
 * Check if an API key exists in the Keychain
 */
export async function hasApiKey(): Promise<boolean> {
  return invoke('has_api_key');
}

/**
 * Delete the API key from the Keychain
 */
export async function deleteApiKey(): Promise<void> {
  return invoke('delete_api_key');
}

/**
 * Validate API key format without storing it
 * Returns true if the key has the correct prefix and length
 */
export async function validateApiKeyFormat(apiKey: string): Promise<boolean> {
  return invoke('validate_api_key_format', { apiKey });
}

/**
 * Store a purchase license key locally.
 */
export async function storeLicenseKey(licenseKey: string): Promise<void> {
  return invoke('store_license_key', { licenseKey });
}

/**
 * Check whether a license key exists.
 */
export async function hasLicenseKey(): Promise<boolean> {
  return invoke('has_license_key');
}

/**
 * Remove stored license key.
 */
export async function deleteLicenseKey(): Promise<void> {
  return invoke('delete_license_key');
}

/**
 * Validate license key format without storing it.
 */
export async function validateLicenseKeyFormat(licenseKey: string): Promise<boolean> {
  return invoke('validate_license_key_format', { licenseKey });
}

// =============================================================================
// Phase E - Provider Management
// =============================================================================

/** Get the active AI provider ID (defaults to 'anthropic') */
export async function getActiveProvider(): Promise<string> {
  return invoke('get_active_provider');
}

/** Set the active AI provider */
export async function setActiveProvider(providerId: string): Promise<void> {
  return invoke('set_active_provider', { providerId });
}

/** List all available AI providers */
export async function listProviders(): Promise<ProviderInfo[]> {
  return invoke('list_providers');
}

/** Validate an API key format for a specific provider */
export async function validateProviderApiKeyFormat(
  providerId: string,
  apiKey: string
): Promise<boolean> {
  return invoke('validate_provider_api_key_format', { providerId, apiKey });
}

/** Store an API key for a specific provider in Keychain */
export async function storeProviderApiKey(
  providerId: string,
  apiKey: string
): Promise<void> {
  return invoke('store_provider_api_key', { providerId, apiKey });
}

/** Check if an API key exists for a specific provider */
export async function hasProviderApiKey(providerId: string): Promise<boolean> {
  return invoke('has_provider_api_key', { providerId });
}

/** Delete the API key for a specific provider */
export async function deleteProviderApiKey(providerId: string): Promise<void> {
  return invoke('delete_provider_api_key', { providerId });
}

/** Check if ANY provider has an API key stored (for onboarding completion) */
export async function hasAnyProviderApiKey(): Promise<boolean> {
  return invoke('has_any_provider_api_key');
}

// =============================================================================
// Phase 1.4 - Chat Commands
// =============================================================================

export interface ChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

export interface ChatResponse {
  content: string;
  input_tokens: number;
  output_tokens: number;
}

/**
 * Send messages to Claude and get a response (non-streaming)
 * @param messages Array of conversation messages
 * @param systemPrompt Optional system prompt for context
 */
export async function sendChatMessage(
  messages: ChatMessage[],
  systemPrompt?: string
): Promise<ChatResponse> {
  return invoke('send_chat_message', {
    messages,
    systemPrompt: systemPrompt ?? null
  });
}

/**
 * Send messages to Claude with streaming response
 * Listen for "chat-stream" events for response chunks
 * @param messages Array of conversation messages
 * @param systemPrompt Optional system prompt for context
 * @param aggregates V2.1.4: Optional org aggregates for answer verification
 * @param queryType V2.1.4: Optional query type for answer verification
 */
export async function sendChatMessageStreaming(
  messages: ChatMessage[],
  systemPrompt?: string,
  aggregates?: OrgAggregates | null,
  queryType?: QueryType | null
): Promise<void> {
  return invoke('send_chat_message_streaming', {
    messages,
    systemPrompt: systemPrompt ?? null,
    aggregates: aggregates ?? null,
    queryType: queryType ?? null,
  });
}

/** Event payload for streaming chunks */
export interface StreamChunk {
  chunk: string;
  done: boolean;
  /** V2.1.4: Verification result - only present when done=true */
  verification?: VerificationResult;
}

// =============================================================================
// Phase 1.5 - Network Detection
// =============================================================================

/**
 * Network status result from the Rust backend
 */
export interface NetworkStatus {
  /** Whether the network is available */
  is_online: boolean;
  /** Whether the Anthropic API is specifically reachable */
  api_reachable: boolean;
  /** Optional error message if offline */
  error_message: string | null;
}

/**
 * Check network connectivity and Anthropic API availability
 * Returns detailed status including error messages
 */
export async function checkNetworkStatus(): Promise<NetworkStatus> {
  return invoke('check_network_status');
}

/**
 * Quick check if online (returns just a boolean)
 * Use for simple online/offline checks
 */
export async function isOnline(): Promise<boolean> {
  return invoke('is_online');
}

// =============================================================================
// Phase 2.2 - Company Profile
// =============================================================================

/**
 * Check if a company profile has been set up
 * Used to gate the app until company profile is configured
 */
export async function hasCompany(): Promise<boolean> {
  return invoke('has_company');
}

/**
 * Get the company profile
 * @throws Error if company profile doesn't exist
 */
export async function getCompany(): Promise<Company> {
  return invoke('get_company');
}

/**
 * Create or update the company profile (upsert)
 * @param input Company name, state (2-letter code), and optional industry
 */
export async function upsertCompany(input: UpsertCompany): Promise<Company> {
  return invoke('upsert_company', { input });
}

/**
 * Get summary of employee work states (operational footprint)
 * Derived from employees table, not company profile
 * Useful for understanding where employees are located
 */
export async function getEmployeeWorkStates(): Promise<EmployeeStatesSummary> {
  return invoke('get_employee_work_states');
}

// =============================================================================
// Phase 2.1 - Employee Management
// =============================================================================

/**
 * Input for creating a new employee
 */
export interface CreateEmployeeInput {
  email: string;
  full_name: string;
  department?: string;
  job_title?: string;
  manager_id?: string;
  hire_date?: string;
  work_state?: string;
  status?: 'active' | 'terminated' | 'leave';
  date_of_birth?: string;
  gender?: string;
  ethnicity?: string;
  termination_date?: string;
  termination_reason?: string;
  extra_fields?: string;
}

/**
 * Input for updating an employee (all fields optional)
 */
export interface UpdateEmployeeInput {
  email?: string;
  full_name?: string;
  department?: string;
  job_title?: string;
  manager_id?: string;
  hire_date?: string;
  work_state?: string;
  status?: 'active' | 'terminated' | 'leave';
  date_of_birth?: string;
  gender?: string;
  ethnicity?: string;
  termination_date?: string;
  termination_reason?: string;
  extra_fields?: string;
}

/**
 * Filter options for listing employees
 */
export interface EmployeeFilter {
  status?: 'active' | 'terminated' | 'leave';
  department?: string;
  work_state?: string;
  search?: string;
  // V2.3.2l: Additional filters for drilldown
  gender?: string;
  ethnicity?: string;
}

/**
 * Result from listing employees (includes pagination info)
 */
export interface EmployeeListResult {
  employees: Employee[];
  total: number;
}

export interface EmployeeWithLatestRating extends Employee {
  latestRating?: PerformanceRating;
}

export interface EmployeeListWithRatingsResult {
  employees: EmployeeWithLatestRating[];
  total: number;
}

/**
 * Result from bulk import operation
 */
export interface ImportResult {
  created: number;
  updated: number;
  errors: string[];
}

/**
 * Create a new employee
 */
export async function createEmployee(input: CreateEmployeeInput): Promise<Employee> {
  return invoke('create_employee', { input });
}

/**
 * Get an employee by ID
 */
export async function getEmployee(id: string): Promise<Employee> {
  return invoke('get_employee', { id });
}

/**
 * Get an employee by email
 */
export async function getEmployeeByEmail(email: string): Promise<Employee | null> {
  return invoke('get_employee_by_email', { email });
}

/**
 * Update an employee
 */
export async function updateEmployee(id: string, input: UpdateEmployeeInput): Promise<Employee> {
  return invoke('update_employee', { id, input });
}

/**
 * Delete an employee
 */
export async function deleteEmployee(id: string): Promise<void> {
  return invoke('delete_employee', { id });
}

/**
 * List employees with optional filtering and pagination
 * @param filter - Optional filter criteria
 * @param limit - Max results (default 100)
 * @param offset - Pagination offset (default 0)
 */
export async function listEmployees(
  filter: EmployeeFilter = {},
  limit?: number,
  offset?: number
): Promise<EmployeeListResult> {
  return invoke('list_employees', { filter, limit, offset });
}

/**
 * List employees with latest performance rating in one backend call
 */
export async function listEmployeesWithRatings(
  filter: EmployeeFilter = {},
  limit?: number,
  offset?: number
): Promise<EmployeeListWithRatingsResult> {
  return invoke('list_employees_with_ratings', { filter, limit, offset });
}

/**
 * Get all unique departments
 */
export async function getDepartments(): Promise<string[]> {
  return invoke('get_departments');
}

/**
 * Get employee counts grouped by status
 * Returns array of [status, count] tuples
 */
export async function getEmployeeCounts(): Promise<[string, number][]> {
  return invoke('get_employee_counts');
}

/**
 * Bulk import employees (upsert by email)
 * Existing employees (matched by email) will be updated
 * New employees will be created
 */
export async function importEmployees(employees: CreateEmployeeInput[]): Promise<ImportResult> {
  return invoke('import_employees', { employees });
}

// =============================================================================
// Phase 2.1 - Review Cycles
// =============================================================================

/**
 * Input for creating a review cycle
 */
export interface CreateReviewCycleInput {
  name: string;
  cycle_type: 'annual' | 'semi-annual' | 'quarterly';
  start_date: string;
  end_date: string;
  status?: 'active' | 'closed';
}

/**
 * Input for updating a review cycle
 */
export interface UpdateReviewCycleInput {
  name?: string;
  cycle_type?: 'annual' | 'semi-annual' | 'quarterly';
  start_date?: string;
  end_date?: string;
  status?: 'active' | 'closed';
}

/**
 * Create a new review cycle
 */
export async function createReviewCycle(input: CreateReviewCycleInput): Promise<ReviewCycle> {
  return invoke('create_review_cycle', { input });
}

/**
 * Get a review cycle by ID
 */
export async function getReviewCycle(id: string): Promise<ReviewCycle> {
  return invoke('get_review_cycle', { id });
}

/**
 * Update a review cycle
 */
export async function updateReviewCycle(id: string, input: UpdateReviewCycleInput): Promise<ReviewCycle> {
  return invoke('update_review_cycle', { id, input });
}

/**
 * Delete a review cycle
 */
export async function deleteReviewCycle(id: string): Promise<void> {
  return invoke('delete_review_cycle', { id });
}

/**
 * List all review cycles, optionally filtered by status
 */
export async function listReviewCycles(statusFilter?: 'active' | 'closed'): Promise<ReviewCycle[]> {
  return invoke('list_review_cycles', { statusFilter });
}

/**
 * Get the current active review cycle (most recent by start_date)
 */
export async function getActiveReviewCycle(): Promise<ReviewCycle | null> {
  return invoke('get_active_review_cycle');
}

/**
 * Close a review cycle (convenience method)
 */
export async function closeReviewCycle(id: string): Promise<ReviewCycle> {
  return invoke('close_review_cycle', { id });
}

// =============================================================================
// Phase 2.1 - Performance Ratings
// =============================================================================

/**
 * Input for creating a performance rating
 */
export interface CreateRatingInput {
  employee_id: string;
  review_cycle_id: string;
  overall_rating: number; // 1.0 - 5.0
  goals_rating?: number;
  competencies_rating?: number;
  reviewer_id?: string;
  rating_date?: string;
}

/**
 * Input for updating a performance rating
 */
export interface UpdateRatingInput {
  overall_rating?: number;
  goals_rating?: number;
  competencies_rating?: number;
  reviewer_id?: string;
  rating_date?: string;
}

/**
 * Rating distribution for analytics
 */
export interface RatingDistribution {
  exceptional: number;    // 5.0
  exceeds: number;        // 4.0-4.9
  meets: number;          // 3.0-3.9
  developing: number;     // 2.0-2.9
  unsatisfactory: number; // 1.0-1.9
  total: number;
}

/**
 * Create a performance rating
 */
export async function createPerformanceRating(input: CreateRatingInput): Promise<PerformanceRating> {
  return invoke('create_performance_rating', { input });
}

/**
 * Get a rating by ID
 */
export async function getPerformanceRating(id: string): Promise<PerformanceRating> {
  return invoke('get_performance_rating', { id });
}

/**
 * Get all ratings for an employee (ordered by cycle date desc)
 */
export async function getRatingsForEmployee(employeeId: string): Promise<PerformanceRating[]> {
  return invoke('get_ratings_for_employee', { employeeId });
}

/**
 * Get all ratings for a review cycle
 */
export async function getRatingsForCycle(reviewCycleId: string): Promise<PerformanceRating[]> {
  return invoke('get_ratings_for_cycle', { reviewCycleId });
}

/**
 * Get the most recent rating for an employee
 */
export async function getLatestRating(employeeId: string): Promise<PerformanceRating | null> {
  return invoke('get_latest_rating', { employeeId });
}

/**
 * Update a performance rating
 */
export async function updatePerformanceRating(id: string, input: UpdateRatingInput): Promise<PerformanceRating> {
  return invoke('update_performance_rating', { id, input });
}

/**
 * Delete a performance rating
 */
export async function deletePerformanceRating(id: string): Promise<void> {
  return invoke('delete_performance_rating', { id });
}

/**
 * Get rating distribution for a cycle (for analytics)
 */
export async function getRatingDistribution(reviewCycleId: string): Promise<RatingDistribution> {
  return invoke('get_rating_distribution', { reviewCycleId });
}

/**
 * Get average rating for a cycle
 */
export async function getAverageRating(reviewCycleId: string): Promise<number | null> {
  return invoke('get_average_rating', { reviewCycleId });
}

// =============================================================================
// Phase 2.1 - Performance Reviews
// =============================================================================

/**
 * Get all performance reviews for an employee (ordered by cycle date desc)
 */
export async function getReviewsForEmployee(employeeId: string): Promise<PerformanceReview[]> {
  return invoke('get_reviews_for_employee', { employeeId });
}

// =============================================================================
// Phase 2.1 - eNPS Responses
// =============================================================================

/**
 * Get all eNPS responses for an employee (ordered by survey date desc)
 */
export async function getEnpsForEmployee(employeeId: string): Promise<EnpsResponse[]> {
  return invoke('get_enps_for_employee', { employeeId });
}

/**
 * Get the most recent eNPS response for an employee
 */
export async function getLatestEnpsForEmployee(employeeId: string): Promise<EnpsResponse | null> {
  return invoke('get_latest_enps_for_employee', { employeeId });
}

// =============================================================================
// Phase 2.3 - Context Builder
// =============================================================================

/**
 * Rating info for employee context
 */
export interface RatingInfo {
  cycle_name: string;
  overall_rating: number;
  rating_date: string | null;
}

/**
 * eNPS info for employee context
 */
export interface EnpsInfo {
  score: number;
  survey_name: string | null;
  survey_date: string;
  feedback: string | null;
}

/**
 * Full employee context including performance and eNPS data
 */
export interface EmployeeContext {
  id: string;
  full_name: string;
  email: string;
  department: string | null;
  job_title: string | null;
  hire_date: string | null;
  work_state: string | null;
  status: string;
  manager_name: string | null;

  // Performance data
  latest_rating: number | null;
  latest_rating_cycle: string | null;
  rating_trend: string | null; // "improving" | "stable" | "declining"
  all_ratings: RatingInfo[];

  // eNPS data
  latest_enps: number | null;
  latest_enps_date: string | null;
  enps_trend: string | null;
  all_enps: EnpsInfo[];
}

/**
 * Company context for system prompt
 */
export interface CompanyContext {
  name: string;
  state: string;
  industry: string | null;
  employee_count: number;
  department_count: number;
}

/**
 * Aggregate eNPS calculation result
 */
export interface EnpsAggregate {
  /** eNPS score (-100 to +100) */
  score: number;
  /** Number of promoters (score >= 9) */
  promoters: number;
  /** Number of passives (score 7-8) */
  passives: number;
  /** Number of detractors (score <= 6) */
  detractors: number;
  /** Total survey responses */
  total_responses: number;
  /** Response rate vs active employees (percentage) */
  response_rate: number;
}

/**
 * Full chat context for building system prompt
 */
export interface ChatContext {
  company: CompanyContext | null;
  employees: EmployeeContext[];
  employee_ids_used: string[];
  memory_summaries: string[];
}

/**
 * Build chat context for a user message
 * Extracts mentions, finds relevant employees, and gathers company data
 * @param userMessage - The user's message to analyze
 * @param selectedEmployeeId - Optional employee ID to prioritize (always included first)
 */
export async function buildChatContext(
  userMessage: string,
  selectedEmployeeId?: string | null
): Promise<ChatContext> {
  return invoke('build_chat_context', {
    userMessage,
    selectedEmployeeId: selectedEmployeeId ?? null
  });
}

/**
 * Get the system prompt for a chat message
 * V2.1.4: Now returns SystemPromptResult with aggregates and query_type for verification
 * @param userMessage - The user's message to analyze
 * @param selectedEmployeeId - Optional employee ID to prioritize (always included first)
 * @returns SystemPromptResult containing prompt, employee IDs, aggregates, and query type
 */
export async function getSystemPrompt(
  userMessage: string,
  selectedEmployeeId?: string | null
): Promise<SystemPromptResult> {
  return invoke('get_system_prompt', {
    userMessage,
    selectedEmployeeId: selectedEmployeeId ?? null
  });
}

/**
 * Get full context for a specific employee
 * Useful for debugging or displaying employee details
 * @param employeeId - The employee ID
 */
export async function getEmployeeContext(employeeId: string): Promise<EmployeeContext> {
  return invoke('get_employee_context', { employeeId });
}

/**
 * Get company context (name, state, employee/department counts)
 */
export async function getCompanyContext(): Promise<CompanyContext | null> {
  return invoke('get_company_context');
}

/**
 * Get aggregate eNPS score for the organization
 * Calculates promoters, passives, detractors, and overall score
 */
export async function getAggregateEnps(): Promise<EnpsAggregate> {
  return invoke('get_aggregate_enps');
}

// =============================================================================
// Phase 2.3 - Settings
// =============================================================================

/**
 * Get a setting value by key
 * Returns null if the setting doesn't exist
 * @param key - The setting key (e.g., "user_name")
 */
export async function getSetting(key: string): Promise<string | null> {
  return invoke('get_setting', { key });
}

/**
 * Set a setting value (creates or updates)
 * @param key - The setting key
 * @param value - The value to store
 */
export async function setSetting(key: string, value: string): Promise<void> {
  return invoke('set_setting', { key, value });
}

/**
 * Delete a setting by key
 * Does nothing if the setting doesn't exist
 */
export async function deleteSetting(key: string): Promise<void> {
  return invoke('delete_setting', { key });
}

/**
 * Check if a setting exists
 */
export async function hasSetting(key: string): Promise<boolean> {
  return invoke('has_setting', { key });
}

// =============================================================================
// V2.1.3 - Personas
// =============================================================================

/**
 * HR persona for customizing Claude's communication style
 */
export interface Persona {
  id: string;
  name: string;
  style: string;
  best_for: string;
  preamble: string;
  communication_style: string;
  sample_response: string;
}

/**
 * Get all available HR personas for the persona switcher
 */
export async function getPersonas(): Promise<Persona[]> {
  return invoke('get_personas');
}

/**
 * Get the app data directory path (where SQLite database is stored)
 * Returns path like ~/Library/Application Support/com.hrcommand.app/
 */
export async function getDataPath(): Promise<string> {
  return invoke('get_data_path');
}

// =============================================================================
// Phase 4.4 - Monday Digest
// =============================================================================

/**
 * Employee data for the Monday Digest (simplified for display)
 */
export interface DigestEmployee {
  id: string;
  full_name: string;
  department?: string;
  hire_date: string;
  /** Years of tenure (for anniversaries) */
  years_tenure?: number;
  /** Days since hire (for new hires) */
  days_since_start?: number;
}

/**
 * Data for the Monday Digest
 */
export interface DigestData {
  /** Employees with work anniversaries this week (within 7 days) */
  anniversaries: DigestEmployee[];
  /** New hires (hired within last 90 days) */
  new_hires: DigestEmployee[];
}

/**
 * Get Monday Digest data (anniversaries and new hires)
 * Returns employees with anniversaries within 7 days and new hires within 90 days
 */
export async function getDigestData(): Promise<DigestData> {
  return invoke('get_digest_data');
}

// =============================================================================
// Phase 2.4 - Cross-Conversation Memory
// =============================================================================

/**
 * A past conversation summary for cross-conversation memory
 */
export interface ConversationSummary {
  conversation_id: string;
  summary: string;
  created_at: string;
}

/**
 * Generate a summary for a conversation using Claude
 * @param messagesJson - JSON string of conversation messages
 * @returns The generated 2-3 sentence summary
 */
export async function generateConversationSummary(messagesJson: string): Promise<string> {
  return invoke('generate_conversation_summary', { messagesJson });
}

/**
 * Save a summary to an existing conversation
 * @param conversationId - The conversation ID to update
 * @param summary - The summary text to save
 */
export async function saveConversationSummary(
  conversationId: string,
  summary: string
): Promise<void> {
  return invoke('save_conversation_summary', { conversationId, summary });
}

/**
 * Search for relevant past conversation memories
 * @param query - The search query (usually the user's message)
 * @param limit - Max number of results (default: 3)
 */
export async function searchMemories(
  query: string,
  limit?: number
): Promise<ConversationSummary[]> {
  return invoke('search_memories', { query, limit });
}

// =============================================================================
// Phase 2.5 - Conversation Management
// =============================================================================

/**
 * Full conversation record from database
 */
export interface ConversationRecord {
  id: string;
  title: string | null;
  summary: string | null;
  messages_json: string;
  created_at: string;
  updated_at: string;
}

/**
 * Lightweight conversation item for sidebar list
 */
export interface ConversationListItem {
  id: string;
  title: string | null;
  summary: string | null;
  message_count: number;
  first_message_preview: string | null;
  created_at: string;
  updated_at: string;
}

/**
 * Input for creating a conversation
 */
export interface CreateConversationInput {
  id: string;
  title?: string;
  messages_json?: string;
}

/**
 * Input for updating a conversation
 */
export interface UpdateConversationInput {
  title?: string;
  messages_json?: string;
  summary?: string;
}

/**
 * Create a new conversation
 * @param input - Conversation ID and optional title/messages
 */
export async function createConversation(
  input: CreateConversationInput
): Promise<ConversationRecord> {
  return invoke('create_conversation', { input });
}

/**
 * Get a conversation by ID
 * @param id - The conversation ID
 */
export async function getConversation(id: string): Promise<ConversationRecord> {
  return invoke('get_conversation', { id });
}

/**
 * Update a conversation (title, messages, or summary)
 * Creates the conversation if it doesn't exist (upsert behavior)
 * @param id - The conversation ID
 * @param input - Fields to update
 */
export async function updateConversation(
  id: string,
  input: UpdateConversationInput
): Promise<ConversationRecord> {
  return invoke('update_conversation', { id, input });
}

/**
 * List conversations for sidebar display
 * Returns lightweight items sorted by updated_at (most recent first)
 * @param limit - Max results (default: 50)
 * @param offset - Pagination offset (default: 0)
 */
export async function listConversations(
  limit?: number,
  offset?: number
): Promise<ConversationListItem[]> {
  return invoke('list_conversations', { limit, offset });
}

/**
 * Search conversations using full-text search
 * Searches across title, messages, and summary
 * @param query - Search query
 * @param limit - Max results (default: 20)
 */
export async function searchConversations(
  query: string,
  limit?: number
): Promise<ConversationListItem[]> {
  return invoke('search_conversations', { query, limit });
}

/**
 * Delete a conversation
 * @param id - The conversation ID to delete
 */
export async function deleteConversation(id: string): Promise<void> {
  return invoke('delete_conversation', { id });
}

/**
 * Generate a title for a conversation using Claude
 * Falls back to truncated first message if Claude fails
 * @param firstMessage - The first user message in the conversation
 */
export async function generateConversationTitle(
  firstMessage: string
): Promise<string> {
  return invoke('generate_conversation_title', { firstMessage });
}

// =============================================================================
// Commands to be implemented in later phases:
// =============================================================================

// Phase 3.1 - PII Scanner
// export async function scanPII(text: string): Promise<PIIRedaction[]>

// =============================================================================
// Phase 2.1.B - File Parsing
// =============================================================================

/**
 * Parse a file (CSV, TSV, XLSX, XLS) and return all rows
 * @param data - Raw file bytes as Uint8Array
 * @param fileName - Original filename (used for format detection)
 */
export async function parseFile(data: Uint8Array, fileName: string): Promise<ParseResult> {
  return invoke('parse_file', { data: Array.from(data), fileName });
}

/**
 * Parse a file and return only a preview (first N rows)
 * Useful for showing users what they're importing before committing
 * @param data - Raw file bytes as Uint8Array
 * @param fileName - Original filename
 * @param previewRows - Number of rows to include (default: 5)
 */
export async function parseFilePreview(
  data: Uint8Array,
  fileName: string,
  previewRows?: number
): Promise<ParsePreview> {
  return invoke('parse_file_preview', { data: Array.from(data), fileName, previewRows });
}

/**
 * Get list of supported file extensions
 */
export async function getSupportedExtensions(): Promise<string[]> {
  return invoke('get_supported_extensions');
}

/**
 * Check if a file is supported for import
 */
export async function isSupportedFile(fileName: string): Promise<boolean> {
  return invoke('is_supported_file', { fileName });
}

/**
 * Map parsed headers to standard employee fields
 * Returns a mapping of standard field name -> parsed header name
 */
export async function mapEmployeeColumns(headers: string[]): Promise<ColumnMapping> {
  return invoke('map_employee_columns', { headers });
}

/**
 * Map parsed headers to performance rating fields
 */
export async function mapRatingColumns(headers: string[]): Promise<ColumnMapping> {
  return invoke('map_rating_columns', { headers });
}

/**
 * Map parsed headers to eNPS fields
 */
export async function mapEnpsColumns(headers: string[]): Promise<ColumnMapping> {
  return invoke('map_enps_columns', { headers });
}

/**
 * Helper to read a File object as Uint8Array for parsing
 */
export async function readFileAsBytes(file: File): Promise<Uint8Array> {
  const buffer = await file.arrayBuffer();
  return new Uint8Array(buffer);
}

// =============================================================================
// Bulk Import Commands (Test Data)
// =============================================================================

/**
 * Result from bulk import operations
 */
export interface BulkImportResult {
  inserted: number;
  errors: string[];
}

/**
 * Result from data integrity verification
 */
export interface IntegrityCheckResult {
  check_name: string;
  passed: boolean;
  expected: number;
  actual: number;
  details: string | null;
}

/**
 * Employee import record (with explicit ID for FK preservation)
 * Note: Uses null union types to match JSON data from generators
 */
export interface ImportEmployee {
  id: string;
  email: string;
  full_name: string;
  department?: string | null;
  job_title?: string | null;
  manager_id?: string | null;
  hire_date?: string | null;
  work_state?: string | null;
  status?: string | null;
  date_of_birth?: string | null;
  gender?: string | null;
  ethnicity?: string | null;
  termination_date?: string | null;
  termination_reason?: string | null;
}

/**
 * Review cycle import record (with explicit ID)
 */
export interface ImportReviewCycle {
  id: string;
  name: string;
  cycle_type: string;
  start_date: string;
  end_date: string;
  status: string;
}

/**
 * Performance rating import record (with explicit ID)
 */
export interface ImportRating {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  reviewer_id?: string;
  overall_rating: number;
  goals_rating?: number;
  competency_rating?: number;
  submitted_at?: string;
}

/**
 * Performance review import record (with explicit ID)
 */
export interface ImportReview {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  reviewer_id?: string;
  strengths?: string;
  areas_for_improvement?: string;
  accomplishments?: string;
  manager_comments?: string;
  submitted_at?: string;
}

/**
 * eNPS import record (with explicit ID)
 * Note: Uses null union types to match JSON data from generators
 */
export interface ImportEnps {
  id: string;
  employee_id: string;
  survey_date: string;
  survey_name: string;
  score: number;
  feedback_text?: string | null;
  submitted_at?: string | null;
}

/**
 * Clear all data from the database (for test data reset)
 */
export async function bulkClearData(): Promise<void> {
  return invoke('bulk_clear_data');
}

/**
 * Bulk import review cycles with predefined IDs
 */
export async function bulkImportReviewCycles(cycles: ImportReviewCycle[]): Promise<BulkImportResult> {
  return invoke('bulk_import_review_cycles', { cycles });
}

/**
 * Bulk import employees with predefined IDs
 */
export async function bulkImportEmployees(employees: ImportEmployee[]): Promise<BulkImportResult> {
  return invoke('bulk_import_employees', { employees });
}

/**
 * Bulk import performance ratings with predefined IDs
 */
export async function bulkImportRatings(ratings: ImportRating[]): Promise<BulkImportResult> {
  return invoke('bulk_import_ratings', { ratings });
}

/**
 * Bulk import performance reviews with predefined IDs
 */
export async function bulkImportReviews(reviews: ImportReview[]): Promise<BulkImportResult> {
  return invoke('bulk_import_reviews', { reviews });
}

/**
 * Bulk import eNPS responses with predefined IDs
 */
export async function bulkImportEnps(responses: ImportEnps[]): Promise<BulkImportResult> {
  return invoke('bulk_import_enps', { responses });
}

/**
 * Verify data integrity after import
 */
export async function verifyDataIntegrity(): Promise<IntegrityCheckResult[]> {
  return invoke('verify_data_integrity');
}

// =============================================================================
// Phase 3.2 - PII Scanning
// =============================================================================

/** Types of PII that can be detected */
export type PiiType = 'ssn' | 'credit_card' | 'bank_account';

/** A single PII match found in text */
export interface PiiMatch {
  pii_type: PiiType;
  start: number;
  end: number;
  // Note: matched_text is not serialized for security
}

/** Result of scanning and redacting text for PII */
export interface RedactionResult {
  /** Text with PII replaced by placeholders */
  redacted_text: string;
  /** List of detected PII instances */
  matches: PiiMatch[];
  /** Whether any PII was found */
  had_pii: boolean;
  /** Human-readable summary (e.g., "Redacted: 1 SSN, 2 credit cards") */
  summary: string | null;
}

/**
 * Scan text for PII and redact if found
 * Returns the redacted text and summary of what was found
 * Used before sending messages to Claude API
 */
export async function scanPii(text: string): Promise<RedactionResult> {
  return invoke('scan_pii', { text });
}

// =============================================================================
// Phase 3.4 - Audit Logging
// =============================================================================

/**
 * Full audit log entry from database
 */
export interface AuditEntry {
  id: string;
  conversation_id: string | null;
  request_redacted: string;
  response_text: string;
  context_used: string | null; // JSON array of employee IDs
  created_at: string;
}

/**
 * Lightweight audit entry for list display
 */
export interface AuditListItem {
  id: string;
  conversation_id: string | null;
  request_preview: string; // First 100 chars
  response_preview: string; // First 100 chars
  employee_count: number;
  created_at: string;
}

/**
 * Input for creating an audit entry
 */
export interface CreateAuditEntryInput {
  conversation_id?: string;
  request_redacted: string;
  response_text: string;
  employee_ids_used: string[];
}

/**
 * Filter options for listing/exporting audit entries
 */
export interface AuditFilter {
  conversation_id?: string;
  start_date?: string; // ISO 8601 format
  end_date?: string; // ISO 8601 format
}

/**
 * CSV export result
 */
export interface ExportResult {
  csv_content: string;
  row_count: number;
}

/**
 * Create an audit log entry after a Claude API interaction
 * Called by frontend after streaming response completes
 * @param input - Audit entry data (conversation_id, redacted request, response, employee IDs)
 */
export async function createAuditEntry(
  input: CreateAuditEntryInput
): Promise<AuditEntry> {
  return invoke('create_audit_entry', { input });
}

/**
 * Get a single audit entry by ID
 * @param id - The audit entry ID
 */
export async function getAuditEntry(id: string): Promise<AuditEntry> {
  return invoke('get_audit_entry', { id });
}

/**
 * List audit entries with optional filtering
 * Returns lightweight items sorted by created_at (most recent first)
 * @param filter - Optional filter by conversation_id or date range
 * @param limit - Max entries to return (default: 50)
 * @param offset - Number of entries to skip (default: 0)
 */
export async function listAuditEntries(
  filter?: AuditFilter,
  limit?: number,
  offset?: number
): Promise<AuditListItem[]> {
  return invoke('list_audit_entries', { filter, limit, offset });
}

/**
 * Count audit entries matching filter (for pagination)
 * @param filter - Optional filter by conversation_id or date range
 */
export async function countAuditEntries(filter?: AuditFilter): Promise<number> {
  return invoke('count_audit_entries', { filter });
}

/**
 * Export audit log to CSV format
 * Response is truncated to first 500 chars per entry
 * @param filter - Optional filter by conversation_id or date range
 */
export async function exportAuditLog(filter?: AuditFilter): Promise<ExportResult> {
  return invoke('export_audit_log', { filter });
}

// =============================================================================
// Phase 4.3 - Backup & Restore
// =============================================================================

/**
 * Table counts for backup metadata
 */
export interface BackupTableCounts {
  employees: number;
  conversations: number;
  company: number;
  settings: number;
  audit_log: number;
  review_cycles: number;
  performance_ratings: number;
  performance_reviews: number;
  enps_responses: number;
}

/**
 * Metadata about a backup file
 */
export interface BackupMetadata {
  version: string;
  created_at: string; // ISO 8601 format
  app_version: string;
  table_counts: BackupTableCounts;
}

/**
 * Result from exporting a backup
 */
export interface BackupExportResult {
  /** The encrypted backup data as byte array */
  encrypted_data: number[];
  /** Suggested filename for the backup */
  filename: string;
  /** Count of records exported per table */
  table_counts: BackupTableCounts;
}

/**
 * Result from importing a backup
 */
export interface BackupImportResult {
  /** Count of records restored per table */
  restored_counts: BackupTableCounts;
  /** Any warnings encountered during import */
  warnings: string[];
}

/**
 * Export all database tables to an encrypted backup
 * Uses AES-256-GCM encryption with Argon2 key derivation
 * @param password - Password for encryption (minimum 8 characters)
 * @returns Export result with encrypted data and table counts
 */
export async function exportBackup(password: string): Promise<BackupExportResult> {
  return invoke('export_backup', { password });
}

/**
 * Validate a backup file and return its metadata (without importing)
 * Use this to preview a backup before importing
 * @param encryptedData - The encrypted backup data as Uint8Array
 * @param password - Password to decrypt the backup
 * @returns Backup metadata if valid
 * @throws Error if password is wrong or backup is invalid
 */
export async function validateBackup(
  encryptedData: Uint8Array,
  password: string
): Promise<BackupMetadata> {
  return invoke('validate_backup', {
    encryptedData: Array.from(encryptedData),
    password
  });
}

/**
 * Import data from an encrypted backup, replacing all existing data
 * WARNING: This deletes all current data before restoring!
 * @param encryptedData - The encrypted backup data as Uint8Array
 * @param password - Password to decrypt the backup
 * @returns Import result with restored counts and any warnings
 * @throws Error if password is wrong or backup is invalid
 */
export async function importBackup(
  encryptedData: Uint8Array,
  password: string
): Promise<BackupImportResult> {
  return invoke('import_backup', {
    encryptedData: Array.from(encryptedData),
    password
  });
}

/**
 * Helper to read a backup file as Uint8Array for importing
 */
export async function readBackupFileAsBytes(file: File): Promise<Uint8Array> {
  const buffer = await file.arrayBuffer();
  return new Uint8Array(buffer);
}

/**
 * Helper to download encrypted backup data as a file
 * @param data - The encrypted data bytes
 * @param filename - The filename to use
 */
export function downloadBackupFile(data: number[], filename: string): void {
  const blob = new Blob([new Uint8Array(data)], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// =============================================================================
// V2.2.1 - Review Highlights
// =============================================================================

/**
 * Get highlight for a specific review
 * Returns null if no highlight has been extracted yet
 * @param reviewId - The performance review ID
 */
export async function getReviewHighlight(reviewId: string): Promise<ReviewHighlight | null> {
  return invoke('get_review_highlight', { reviewId });
}

/**
 * Get all highlights for an employee
 * @param employeeId - The employee ID
 */
export async function getHighlightsForEmployee(employeeId: string): Promise<ReviewHighlight[]> {
  return invoke('get_highlights_for_employee', { employeeId });
}

/**
 * Extract highlights from a single review using Claude API
 * @param reviewId - The performance review ID to extract from
 */
export async function extractReviewHighlight(reviewId: string): Promise<ReviewHighlight> {
  return invoke('extract_review_highlight', { reviewId });
}

/**
 * Extract highlights for multiple reviews in batch
 * @param reviewIds - Array of review IDs to process
 */
export async function extractHighlightsBatch(reviewIds: string[]): Promise<BatchExtractionResult> {
  return invoke('extract_highlights_batch', { reviewIds });
}

/**
 * Find reviews that need highlights extracted
 * Returns IDs of reviews without existing highlights
 */
export async function findReviewsPendingExtraction(): Promise<string[]> {
  return invoke('find_reviews_pending_extraction');
}

/**
 * Get employee career summary
 * Returns null if no summary has been generated yet
 * @param employeeId - The employee ID
 */
export async function getEmployeeSummary(employeeId: string): Promise<EmployeeSummary | null> {
  return invoke('get_employee_summary', { employeeId });
}

/**
 * Generate employee career summary from their review highlights
 * Requires highlights to be extracted first
 * @param employeeId - The employee ID
 */
export async function generateEmployeeSummary(employeeId: string): Promise<EmployeeSummary> {
  return invoke('generate_employee_summary', { employeeId });
}

/**
 * Invalidate highlight and summary when a review is updated
 * Call this after updating a performance review to trigger re-extraction
 * @param reviewId - The updated review ID
 * @param employeeId - The employee ID (for summary invalidation)
 */
export async function invalidateReviewHighlight(
  reviewId: string,
  employeeId: string
): Promise<void> {
  return invoke('invalidate_review_highlight', { reviewId, employeeId });
}

// =============================================================================
// V2.4.1 - Attention Signals
// =============================================================================

import type {
  AttentionAreasSummary,
  ThemeOccurrence,
} from './signals-types';

// Re-export types for convenience
export type {
  AttentionAreasSummary,
  TeamAttentionSignal,
  AttentionLevel,
  TenureFactor,
  PerformanceFactor,
  EngagementFactor,
  ThemeOccurrence,
} from './signals-types';

/**
 * Check if the attention signals feature is enabled
 */
export async function isSignalsEnabled(): Promise<boolean> {
  return invoke('is_signals_enabled');
}

/**
 * Get team attention signals for all departments
 * Returns teams sorted by attention score, filtered to MIN_TEAM_SIZE (5)
 * @throws Error if signals feature is not enabled
 */
export async function getAttentionSignals(): Promise<AttentionAreasSummary> {
  return invoke('get_attention_signals');
}

/**
 * Get common themes for a specific team from review highlights
 * @param department - The department name
 */
export async function getTeamThemes(department: string): Promise<ThemeOccurrence[]> {
  return invoke('get_team_themes', { department });
}

// =============================================================================
// V2.4.2 - DEI & Fairness Lens
// =============================================================================

import type {
  RepresentationResult,
  RatingParityResult,
  PromotionRatesResult,
  FairnessLensSummary,
  DeiGroupBy,
} from './dei-types';

// Re-export types for convenience
export type {
  DeiBreakdown,
  RepresentationResult,
  RatingParityItem,
  RatingParityResult,
  PromotionRateItem,
  PromotionRatesResult,
  FairnessLensSummary,
  DeiGroupBy,
} from './dei-types';

/**
 * Check if the fairness lens feature is enabled
 */
export async function isFairnessLensEnabled(): Promise<boolean> {
  return invoke('is_fairness_lens_enabled');
}

/**
 * Get representation breakdown by demographic field
 * @param groupBy - "gender" or "ethnicity"
 * @param filterDepartment - Optional department filter
 * @throws Error if fairness lens feature is not enabled
 */
export async function getRepresentationBreakdown(
  groupBy: DeiGroupBy,
  filterDepartment?: string | null
): Promise<RepresentationResult> {
  return invoke('get_representation_breakdown', {
    groupBy,
    filterDepartment: filterDepartment ?? null
  });
}

/**
 * Get rating parity by demographic field
 * @param groupBy - "gender" or "ethnicity"
 * @throws Error if fairness lens feature is not enabled
 */
export async function getRatingParity(
  groupBy: DeiGroupBy
): Promise<RatingParityResult> {
  return invoke('get_rating_parity', { groupBy });
}

/**
 * Get promotion rates by demographic field
 * Infers promotions from job title keywords
 * @param groupBy - "gender" or "ethnicity"
 * @throws Error if fairness lens feature is not enabled
 */
export async function getPromotionRates(
  groupBy: DeiGroupBy
): Promise<PromotionRatesResult> {
  return invoke('get_promotion_rates', { groupBy });
}

/**
 * Get complete fairness lens summary (all DEI metrics)
 * @throws Error if fairness lens feature is not enabled
 */
export async function getFairnessLensSummary(): Promise<FairnessLensSummary> {
  return invoke('get_fairness_lens_summary');
}

// =============================================================================
// V2.5.1 - Data Quality Center
// =============================================================================

import type {
  ImportType,
  HeaderNormalization,
  HrisPreset,
  HrisPresetId,
  ImportValidationResult,
  ValidationIssue,
  DuplicateGroup,
  ParsedRow,
} from './types';

// -- Backend response types (differ from frontend display types) --

interface BackendHeaderAnalysis {
  original: string;
  normalized: string;
  suggested_field: string | null;
  confidence: string; // "exact", "alias", "fuzzy", "none"
  sample_values: string[];
}

interface BackendHeaderAnalysisResult {
  headers: BackendHeaderAnalysis[];
  unmapped_required: string[];
  unmapped_optional: string[];
  import_type: string;
}

interface BackendValidationIssue {
  row_index: number;
  field: string;
  value: string;
  message: string;
  severity: 'error' | 'warning';
  rule: string;
  suggested_fix: string | null;
}

interface BackendValidationResult {
  issues: BackendValidationIssue[];
  error_count: number;
  warning_count: number;
  total_rows: number;
  valid_rows: number;
  can_import: boolean;
}

interface BackendDuplicatePair {
  row_a: number;
  row_b: number;
  match_type: string;
  confidence: number;
  matched_fields: Record<string, [string, string]>;
}

interface BackendDedupeResult {
  duplicates: BackendDuplicatePair[];
  total_rows: number;
  affected_rows: number[];
}

interface BackendHrisPreset {
  id: string;
  name: string;
  description: string;
  header_mappings: [string, string[]][];
}

/** Wraps a flat ColumnMapping into backend's ColumnMappingConfig shape */
function toMappingConfig(mapping: ColumnMapping) {
  return { mappings: mapping, preset_name: null };
}

/** Map confidence string to numeric value */
function confidenceToNumber(c: string): number {
  switch (c) {
    case 'exact': return 1.0;
    case 'alias': return 0.85;
    case 'fuzzy': return 0.7;
    default: return 0.0;
  }
}

/** Map backend rule code to frontend errorType */
function ruleToErrorType(rule: string): ValidationIssue['errorType'] {
  switch (rule) {
    case 'required_field': return 'missing_required';
    case 'invalid_email': return 'invalid_email';
    case 'invalid_date':
    case 'date_format': return 'invalid_date';
    case 'invalid_state': return 'invalid_state';
    case 'invalid_status':
    case 'invalid_rating':
    case 'invalid_enps_score': return 'out_of_range';
    case 'dob_range':
    case 'future_date': return 'out_of_range';
    case 'duplicate_email_in_batch': return 'duplicate_in_file';
    default: return 'unknown_value';
  }
}

/**
 * Analyze headers and auto-detect column mappings.
 * Calls backend analyze_import_headers and transforms to HeaderNormalization[].
 * @param headers - Source headers from parsed file
 * @param dataType - Import type for field matching
 * @param hrisPreset - Optional HRIS preset to apply
 */
export async function normalizeHeaders(
  headers: string[],
  dataType: ImportType,
  hrisPreset?: HrisPresetId | null
): Promise<HeaderNormalization[]> {
  // If a preset is specified, apply it first
  if (hrisPreset) {
    const presetMapping: { mappings: Record<string, string>; preset_name: string | null } | null =
      await invoke('apply_hris_preset', { presetId: hrisPreset, headers });
    if (presetMapping) {
      // Build normalizations from preset mapping
      const reverseMappings = new Map<string, string>();
      for (const [target, source] of Object.entries(presetMapping.mappings)) {
        reverseMappings.set(source, target);
      }
      return headers.map((h) => ({
        original: h,
        normalized: h.toLowerCase().replace(/\s+/g, '_').trim(),
        detectedField: reverseMappings.get(h) ?? null,
        confidence: reverseMappings.has(h) ? 0.9 : 0,
      }));
    }
  }

  // Fall back to analyze_import_headers
  const sampleRows: ParsedRow[] = [];
  const importType = dataType === 'employees' ? 'Employees'
    : dataType === 'ratings' ? 'Ratings'
    : dataType === 'reviews' ? 'Reviews'
    : 'Enps';

  const result: BackendHeaderAnalysisResult = await invoke('analyze_import_headers', {
    headers,
    sampleRows,
    importType,
  });

  return result.headers.map((h) => ({
    original: h.original,
    normalized: h.normalized,
    detectedField: h.suggested_field,
    confidence: confidenceToNumber(h.confidence),
  }));
}

/**
 * Get available HRIS presets.
 * Transforms backend Vec<HrisPreset> (tuple header_mappings) to frontend HrisPreset (Record mappings).
 */
export async function getHrisPresets(): Promise<HrisPreset[]> {
  const raw: BackendHrisPreset[] = await invoke('get_hris_presets');
  return raw.map((p) => ({
    id: p.id as HrisPresetId,
    name: p.name,
    description: p.description,
    mappings: Object.fromEntries(p.header_mappings),
  }));
}

/**
 * Validate import data against schema and business rules.
 * Calls backend validate_import_rows and transforms response.
 */
export async function validateImportData(
  rows: Record<string, string>[],
  mapping: ColumnMapping,
  dataType: ImportType
): Promise<ImportValidationResult> {
  const importType = dataType === 'employees' ? 'Employees'
    : dataType === 'ratings' ? 'Ratings'
    : dataType === 'reviews' ? 'Reviews'
    : 'Enps';

  const result: BackendValidationResult = await invoke('validate_import_rows', {
    rows,
    mapping: toMappingConfig(mapping),
    importType,
  });

  // Transform backend issues to frontend format
  const issues: ValidationIssue[] = result.issues.map((i) => ({
    row: i.row_index + 1, // Backend is 0-based, frontend is 1-based
    column: i.field,
    value: i.value,
    message: i.message,
    severity: i.severity,
    errorType: ruleToErrorType(i.rule),
  }));

  return {
    isValid: result.can_import,
    issues,
    errorRowCount: result.error_count,
    warningRowCount: result.warning_count,
    cleanRowCount: result.valid_rows,
  };
}

/**
 * Detect potential duplicates within import data (in-file duplicates).
 * Calls backend detect_duplicates and transforms DuplicatePair[] to DuplicateGroup[].
 */
export async function detectDuplicates(
  rows: Record<string, string>[],
  mapping: ColumnMapping,
  _dataType: ImportType
): Promise<DuplicateGroup[]> {
  const result: BackendDedupeResult = await invoke('detect_duplicates', {
    rows,
    mapping: toMappingConfig(mapping),
  });

  // Transform in-file duplicate pairs to DuplicateGroup format
  return result.duplicates.map((pair, idx) => {
    const rowA = rows[pair.row_a] ?? {};
    const rowB = rows[pair.row_b] ?? {};

    return {
      id: `dup-${idx}`,
      incoming: rowB,
      existing: {
        id: `row-${pair.row_a}`,
        email: rowA['email'] ?? rowA['employee_email'] ?? '',
        full_name: [rowA['first_name'], rowA['last_name']].filter(Boolean).join(' ') || 'Row ' + (pair.row_a + 1),
        ...rowA,
      },
      matchReason: pair.match_type.replace(/_/g, ' '),
      confidence: pair.confidence,
    };
  });
}

// =============================================================================
// V2.6 - Trial Mode
// =============================================================================

/** Trial status returned by the backend */
export interface TrialStatus {
  is_trial: boolean;
  has_license: boolean;
  has_api_key: boolean;
  messages_used: number;
  messages_limit: number;
  employees_used: number;
  employees_limit: number;
}

/** Result of checking whether more employees can be added */
export interface EmployeeLimitCheck {
  allowed: boolean;
  current: number;
  limit: number;
}

/**
 * Get current trial status (limits, usage counts, whether in trial mode)
 */
export async function getTrialStatus(): Promise<TrialStatus> {
  return invoke('get_trial_status');
}

/**
 * Check if the employee limit allows adding more employees
 */
export async function checkEmployeeLimit(): Promise<EmployeeLimitCheck> {
  return invoke('check_employee_limit');
}

// =============================================================================
// Document Ingestion (V3.0)
// =============================================================================

/** Set the document folder path and trigger initial scan */
export async function setDocumentFolder(path: string): Promise<DocumentFolderStats> {
  return invoke('set_document_folder', { path });
}

/** Remove the document folder and all indexed data */
export async function removeDocumentFolder(): Promise<void> {
  return invoke('remove_document_folder');
}

/** Get the current document folder stats (null if none configured) */
export async function getDocumentFolder(): Promise<DocumentFolderStats | null> {
  return invoke('get_document_folder');
}

/** Trigger a manual re-scan of the document folder */
export async function rescanDocuments(): Promise<DocumentFolderStats> {
  return invoke('rescan_documents');
}

/** Get document indexing stats */
export async function getDocumentStats(): Promise<DocumentStats> {
  return invoke('get_document_stats');
}
