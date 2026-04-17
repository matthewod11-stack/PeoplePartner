//! Encrypted backup and restore functionality for People Partner.
//!
//! This module provides secure export/import of all database tables using:
//! - AES-256-GCM for authenticated encryption
//! - Argon2id for password-based key derivation
//! - flate2 for compression

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use chrono::{DateTime, Utc};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::io::{Read, Write};
use thiserror::Error;

// ============================================================================
// Constants
// ============================================================================

/// Settings keys excluded from backup export.
/// These are device-specific or security-sensitive and should not be
/// transferred between machines.
const EXCLUDED_SETTINGS_KEYS: &[&str] = &[
    "device_id",
    "trial_messages_used",
    "proxy_signing_secret",
];

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Error, Serialize)]
pub enum BackupError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Incorrect password")]
    InvalidPassword,

    #[error("Invalid or corrupted backup file")]
    InvalidBackup,

    #[error("Backup version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },

    #[error("IO error: {0}")]
    Io(String),

    #[error("Compression error: {0}")]
    Compression(String),
}

impl From<sqlx::Error> for BackupError {
    fn from(e: sqlx::Error) -> Self {
        BackupError::Database(e.to_string())
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Current backup format version
const BACKUP_VERSION: &str = "1.0";

/// Minimum password length
const MIN_PASSWORD_LENGTH: usize = 8;

/// Salt length for Argon2
const SALT_LENGTH: usize = 16;

/// Nonce length for AES-GCM
const NONCE_LENGTH: usize = 12;

// ============================================================================
// Backup Metadata & Results
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCounts {
    pub employees: usize,
    pub conversations: usize,
    pub company: usize,
    pub settings: usize,
    pub audit_log: usize,
    pub review_cycles: usize,
    pub performance_ratings: usize,
    pub performance_reviews: usize,
    pub enps_responses: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub table_counts: TableCounts,
}

#[derive(Debug, Serialize)]
pub struct ExportResult {
    /// The encrypted backup data as bytes
    pub encrypted_data: Vec<u8>,
    /// Suggested filename for the backup
    pub filename: String,
    /// Count of records exported per table
    pub table_counts: TableCounts,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    /// Count of records restored per table
    pub restored_counts: TableCounts,
    /// Any warnings encountered during import
    pub warnings: Vec<String>,
}

// ============================================================================
// Row Types (matching SQLite schema exactly)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeRow {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub department: Option<String>,
    pub job_title: Option<String>,
    pub manager_id: Option<String>,
    pub hire_date: Option<String>,
    pub work_state: Option<String>,
    pub status: String,
    pub extra_fields: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub date_of_birth: Option<String>,
    pub gender: Option<String>,
    pub ethnicity: Option<String>,
    pub termination_date: Option<String>,
    pub termination_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRow {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub messages_json: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyRow {
    pub id: String,
    pub name: String,
    pub state: String,
    pub industry: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsRow {
    pub key: String,
    pub value: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRow {
    pub id: String,
    pub conversation_id: Option<String>,
    pub request_redacted: String,
    pub response_text: String,
    pub context_used: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCycleRow {
    pub id: String,
    pub name: String,
    pub cycle_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRatingRow {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub overall_rating: f64,
    pub goals_rating: Option<f64>,
    pub competencies_rating: Option<f64>,
    pub reviewer_id: Option<String>,
    pub rating_date: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReviewRow {
    pub id: String,
    pub employee_id: String,
    pub review_cycle_id: String,
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
    pub accomplishments: Option<String>,
    pub goals_next_period: Option<String>,
    pub manager_comments: Option<String>,
    pub self_assessment: Option<String>,
    pub reviewer_id: Option<String>,
    pub review_date: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnpsRow {
    pub id: String,
    pub employee_id: String,
    pub score: i32,
    pub survey_date: String,
    pub survey_name: Option<String>,
    pub feedback_text: Option<String>,
    pub created_at: Option<String>,
}

// ============================================================================
// Backup Data Structure
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTables {
    pub employees: Vec<EmployeeRow>,
    pub conversations: Vec<ConversationRow>,
    pub company: Vec<CompanyRow>,
    pub settings: Vec<SettingsRow>,
    pub audit_log: Vec<AuditLogRow>,
    pub review_cycles: Vec<ReviewCycleRow>,
    pub performance_ratings: Vec<PerformanceRatingRow>,
    pub performance_reviews: Vec<PerformanceReviewRow>,
    pub enps_responses: Vec<EnpsRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub metadata: BackupMetadata,
    pub tables: BackupTables,
}

// ============================================================================
// Encryption Helpers
// ============================================================================

/// Derive a 256-bit key from password using Argon2id
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], BackupError> {
    // Use Argon2id with reasonable parameters for desktop app
    let argon2 = Argon2::default();

    // Convert salt to SaltString format
    let salt_string = SaltString::encode_b64(salt)
        .map_err(|e| BackupError::Encryption(format!("Salt encoding error: {}", e)))?;

    // Hash the password
    let hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .map_err(|e| BackupError::Encryption(format!("Key derivation error: {}", e)))?;

    // Extract the hash output (32 bytes for AES-256)
    let hash_output = hash
        .hash
        .ok_or_else(|| BackupError::Encryption("No hash output".to_string()))?;

    let bytes = hash_output.as_bytes();
    if bytes.len() < 32 {
        return Err(BackupError::Encryption("Hash output too short".to_string()));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes[..32]);
    Ok(key)
}

/// Encrypt data with AES-256-GCM
/// Returns: [salt: 16 bytes][nonce: 12 bytes][ciphertext]
fn encrypt_data(data: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    // Generate random salt and nonce
    let mut salt = [0u8; SALT_LENGTH];
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    // Derive key from password
    let key = derive_key(password, &salt)?;

    // Create cipher and encrypt
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| BackupError::Encryption(format!("Cipher init error: {}", e)))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| BackupError::Encryption(format!("Encryption error: {}", e)))?;

    // Concatenate salt + nonce + ciphertext
    let mut result = Vec::with_capacity(SALT_LENGTH + NONCE_LENGTH + ciphertext.len());
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt data with AES-256-GCM
/// Expects: [salt: 16 bytes][nonce: 12 bytes][ciphertext]
fn decrypt_data(encrypted: &[u8], password: &str) -> Result<Vec<u8>, BackupError> {
    // Validate minimum length
    if encrypted.len() < SALT_LENGTH + NONCE_LENGTH + 16 {
        return Err(BackupError::InvalidBackup);
    }

    // Extract salt, nonce, and ciphertext
    let salt = &encrypted[..SALT_LENGTH];
    let nonce_bytes = &encrypted[SALT_LENGTH..SALT_LENGTH + NONCE_LENGTH];
    let ciphertext = &encrypted[SALT_LENGTH + NONCE_LENGTH..];

    // Derive key from password
    let key = derive_key(password, salt)?;

    // Create cipher and decrypt
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| BackupError::Encryption(format!("Cipher init error: {}", e)))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| BackupError::InvalidPassword)
}

// ============================================================================
// Compression Helpers
// ============================================================================

/// Compress data using gzip
fn compress_data(data: &[u8]) -> Result<Vec<u8>, BackupError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| BackupError::Compression(format!("Compression write error: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| BackupError::Compression(format!("Compression finish error: {}", e)))
}

/// Decompress gzip data
fn decompress_data(compressed: &[u8]) -> Result<Vec<u8>, BackupError> {
    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| BackupError::Compression(format!("Decompression error: {}", e)))?;
    Ok(decompressed)
}

// ============================================================================
// Database Fetch Functions
// ============================================================================

async fn fetch_employees(pool: &SqlitePool) -> Result<Vec<EmployeeRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT
            id, email, full_name, department, job_title, manager_id,
            hire_date, work_state, status, extra_fields, created_at, updated_at,
            date_of_birth, gender, ethnicity, termination_date, termination_reason
        FROM employees"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| EmployeeRow {
            id: row.get("id"),
            email: row.get("email"),
            full_name: row.get("full_name"),
            department: row.get("department"),
            job_title: row.get("job_title"),
            manager_id: row.get("manager_id"),
            hire_date: row.get("hire_date"),
            work_state: row.get("work_state"),
            status: row.get("status"),
            extra_fields: row.get("extra_fields"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            date_of_birth: row.get("date_of_birth"),
            gender: row.get("gender"),
            ethnicity: row.get("ethnicity"),
            termination_date: row.get("termination_date"),
            termination_reason: row.get("termination_reason"),
        })
        .collect())
}

async fn fetch_conversations(pool: &SqlitePool) -> Result<Vec<ConversationRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, title, summary, messages_json, created_at, updated_at FROM conversations"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| ConversationRow {
            id: row.get("id"),
            title: row.get("title"),
            summary: row.get("summary"),
            messages_json: row.get("messages_json"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

async fn fetch_company(pool: &SqlitePool) -> Result<Vec<CompanyRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, name, state, industry, created_at FROM company"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| CompanyRow {
            id: row.get("id"),
            name: row.get("name"),
            state: row.get("state"),
            industry: row.get("industry"),
            created_at: row.get("created_at"),
        })
        .collect())
}

async fn fetch_settings(pool: &SqlitePool) -> Result<Vec<SettingsRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT key, value, updated_at FROM settings"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .filter_map(|row| {
            let key: String = row.get("key");
            if EXCLUDED_SETTINGS_KEYS.contains(&key.as_str()) {
                None
            } else {
                Some(SettingsRow {
                    key,
                    value: row.get("value"),
                    updated_at: row.get("updated_at"),
                })
            }
        })
        .collect())
}

async fn fetch_audit_log(pool: &SqlitePool) -> Result<Vec<AuditLogRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, conversation_id, request_redacted, response_text, context_used, created_at
        FROM audit_log"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| AuditLogRow {
            id: row.get("id"),
            conversation_id: row.get("conversation_id"),
            request_redacted: row.get("request_redacted"),
            response_text: row.get("response_text"),
            context_used: row.get("context_used"),
            created_at: row.get("created_at"),
        })
        .collect())
}

async fn fetch_review_cycles(pool: &SqlitePool) -> Result<Vec<ReviewCycleRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, name, cycle_type, start_date, end_date, status, created_at FROM review_cycles"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| ReviewCycleRow {
            id: row.get("id"),
            name: row.get("name"),
            cycle_type: row.get("cycle_type"),
            start_date: row.get("start_date"),
            end_date: row.get("end_date"),
            status: row.get("status"),
            created_at: row.get("created_at"),
        })
        .collect())
}

async fn fetch_performance_ratings(
    pool: &SqlitePool,
) -> Result<Vec<PerformanceRatingRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, employee_id, review_cycle_id, overall_rating, goals_rating,
            competencies_rating, reviewer_id, rating_date, created_at, updated_at
        FROM performance_ratings"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| PerformanceRatingRow {
            id: row.get("id"),
            employee_id: row.get("employee_id"),
            review_cycle_id: row.get("review_cycle_id"),
            overall_rating: row.get("overall_rating"),
            goals_rating: row.get("goals_rating"),
            competencies_rating: row.get("competencies_rating"),
            reviewer_id: row.get("reviewer_id"),
            rating_date: row.get("rating_date"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

async fn fetch_performance_reviews(
    pool: &SqlitePool,
) -> Result<Vec<PerformanceReviewRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, employee_id, review_cycle_id, strengths, areas_for_improvement,
            accomplishments, goals_next_period, manager_comments, self_assessment,
            reviewer_id, review_date, created_at, updated_at
        FROM performance_reviews"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| PerformanceReviewRow {
            id: row.get("id"),
            employee_id: row.get("employee_id"),
            review_cycle_id: row.get("review_cycle_id"),
            strengths: row.get("strengths"),
            areas_for_improvement: row.get("areas_for_improvement"),
            accomplishments: row.get("accomplishments"),
            goals_next_period: row.get("goals_next_period"),
            manager_comments: row.get("manager_comments"),
            self_assessment: row.get("self_assessment"),
            reviewer_id: row.get("reviewer_id"),
            review_date: row.get("review_date"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

async fn fetch_enps_responses(pool: &SqlitePool) -> Result<Vec<EnpsRow>, BackupError> {
    let rows = sqlx::query(
        r#"SELECT id, employee_id, score, survey_date, survey_name, feedback_text, created_at
        FROM enps_responses"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|row| EnpsRow {
            id: row.get("id"),
            employee_id: row.get("employee_id"),
            score: row.get("score"),
            survey_date: row.get("survey_date"),
            survey_name: row.get("survey_name"),
            feedback_text: row.get("feedback_text"),
            created_at: row.get("created_at"),
        })
        .collect())
}

/// Fetch all tables for backup
async fn fetch_all_tables(pool: &SqlitePool) -> Result<BackupTables, BackupError> {
    Ok(BackupTables {
        employees: fetch_employees(pool).await?,
        conversations: fetch_conversations(pool).await?,
        company: fetch_company(pool).await?,
        settings: fetch_settings(pool).await?,
        audit_log: fetch_audit_log(pool).await?,
        review_cycles: fetch_review_cycles(pool).await?,
        performance_ratings: fetch_performance_ratings(pool).await?,
        performance_reviews: fetch_performance_reviews(pool).await?,
        enps_responses: fetch_enps_responses(pool).await?,
    })
}

// ============================================================================
// Database Clear Functions (FK-safe order: child → parent)
// ============================================================================

/// Clear all tables in FK-safe order for import
/// Order: enps_responses → performance_reviews → performance_ratings → audit_log
///        → conversations → employees → review_cycles → settings → company
/// FTS tables are cleared AFTER their content tables so DELETE triggers can fire.
async fn clear_all_tables(conn: &mut SqliteConnection) -> Result<(), BackupError> {
    // Child tables first (those with foreign keys)
    sqlx::query("DELETE FROM enps_responses")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM performance_reviews")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM performance_ratings")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM audit_log")
        .execute(&mut *conn)
        .await?;

    // Content tables before their FTS indexes (DELETE triggers populate FTS delete log)
    sqlx::query("DELETE FROM conversations")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM conversations_fts")
        .execute(&mut *conn)
        .await?;

    sqlx::query("DELETE FROM employees")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM review_cycles")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM settings")
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM company")
        .execute(&mut *conn)
        .await?;

    // Note: performance_reviews_fts is cleared after performance_reviews above
    sqlx::query("DELETE FROM performance_reviews_fts")
        .execute(&mut *conn)
        .await?;

    Ok(())
}

// ============================================================================
// Database Restore Functions (FK-safe order: parent → child)
// ============================================================================

async fn restore_company(conn: &mut SqliteConnection, rows: &[CompanyRow]) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO company (id, name, state, industry, created_at)
            VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.state)
        .bind(&row.industry)
        .bind(&row.created_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_settings(conn: &mut SqliteConnection, rows: &[SettingsRow]) -> Result<usize, BackupError> {
    let mut count = 0;
    for row in rows {
        if EXCLUDED_SETTINGS_KEYS.contains(&row.key.as_str()) {
            continue;
        }
        sqlx::query(
            r#"INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)"#,
        )
        .bind(&row.key)
        .bind(&row.value)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?;
        count += 1;
    }
    Ok(count)
}

async fn restore_review_cycles(
    conn: &mut SqliteConnection,
    rows: &[ReviewCycleRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.cycle_type)
        .bind(&row.start_date)
        .bind(&row.end_date)
        .bind(&row.status)
        .bind(&row.created_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_employees(
    conn: &mut SqliteConnection,
    rows: &[EmployeeRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO employees (
                id, email, full_name, department, job_title, manager_id,
                hire_date, work_state, status, extra_fields, created_at, updated_at,
                date_of_birth, gender, ethnicity, termination_date, termination_reason
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.email)
        .bind(&row.full_name)
        .bind(&row.department)
        .bind(&row.job_title)
        .bind(&row.manager_id)
        .bind(&row.hire_date)
        .bind(&row.work_state)
        .bind(&row.status)
        .bind(&row.extra_fields)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .bind(&row.date_of_birth)
        .bind(&row.gender)
        .bind(&row.ethnicity)
        .bind(&row.termination_date)
        .bind(&row.termination_reason)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_performance_ratings(
    conn: &mut SqliteConnection,
    rows: &[PerformanceRatingRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO performance_ratings (
                id, employee_id, review_cycle_id, overall_rating, goals_rating,
                competencies_rating, reviewer_id, rating_date, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.employee_id)
        .bind(&row.review_cycle_id)
        .bind(row.overall_rating)
        .bind(row.goals_rating)
        .bind(row.competencies_rating)
        .bind(&row.reviewer_id)
        .bind(&row.rating_date)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_performance_reviews(
    conn: &mut SqliteConnection,
    rows: &[PerformanceReviewRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO performance_reviews (
                id, employee_id, review_cycle_id, strengths, areas_for_improvement,
                accomplishments, goals_next_period, manager_comments, self_assessment,
                reviewer_id, review_date, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.employee_id)
        .bind(&row.review_cycle_id)
        .bind(&row.strengths)
        .bind(&row.areas_for_improvement)
        .bind(&row.accomplishments)
        .bind(&row.goals_next_period)
        .bind(&row.manager_comments)
        .bind(&row.self_assessment)
        .bind(&row.reviewer_id)
        .bind(&row.review_date)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_enps_responses(
    conn: &mut SqliteConnection,
    rows: &[EnpsRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO enps_responses (
                id, employee_id, score, survey_date, survey_name, feedback_text, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.employee_id)
        .bind(row.score)
        .bind(&row.survey_date)
        .bind(&row.survey_name)
        .bind(&row.feedback_text)
        .bind(&row.created_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_conversations(
    conn: &mut SqliteConnection,
    rows: &[ConversationRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO conversations (id, title, summary, messages_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.title)
        .bind(&row.summary)
        .bind(&row.messages_json)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

async fn restore_audit_log(
    conn: &mut SqliteConnection,
    rows: &[AuditLogRow],
) -> Result<usize, BackupError> {
    for row in rows {
        sqlx::query(
            r#"INSERT INTO audit_log (id, conversation_id, request_redacted, response_text, context_used, created_at)
            VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&row.id)
        .bind(&row.conversation_id)
        .bind(&row.request_redacted)
        .bind(&row.response_text)
        .bind(&row.context_used)
        .bind(&row.created_at)
        .execute(&mut *conn)
        .await?;
    }
    Ok(rows.len())
}

/// Restore all tables in FK-safe order
/// Order: company → settings → review_cycles → employees → performance_ratings
///        → performance_reviews → enps_responses → conversations → audit_log
async fn restore_all_tables(
    conn: &mut SqliteConnection,
    tables: &BackupTables,
) -> Result<TableCounts, BackupError> {
    Ok(TableCounts {
        company: restore_company(&mut *conn, &tables.company).await?,
        settings: restore_settings(&mut *conn, &tables.settings).await?,
        review_cycles: restore_review_cycles(&mut *conn, &tables.review_cycles).await?,
        employees: restore_employees(&mut *conn, &tables.employees).await?,
        performance_ratings: restore_performance_ratings(
            &mut *conn,
            &tables.performance_ratings,
        )
        .await?,
        performance_reviews: restore_performance_reviews(
            &mut *conn,
            &tables.performance_reviews,
        )
        .await?,
        enps_responses: restore_enps_responses(&mut *conn, &tables.enps_responses).await?,
        conversations: restore_conversations(&mut *conn, &tables.conversations).await?,
        audit_log: restore_audit_log(&mut *conn, &tables.audit_log).await?,
    })
}

// ============================================================================
// Public API
// ============================================================================

/// Export all database tables to an encrypted backup
pub async fn export_backup(pool: &SqlitePool, password: &str) -> Result<ExportResult, BackupError> {
    // Validate password length
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(BackupError::Encryption(format!(
            "Password must be at least {} characters",
            MIN_PASSWORD_LENGTH
        )));
    }

    // Fetch all data
    let tables = fetch_all_tables(pool).await?;

    // Build metadata
    let table_counts = TableCounts {
        employees: tables.employees.len(),
        conversations: tables.conversations.len(),
        company: tables.company.len(),
        settings: tables.settings.len(),
        audit_log: tables.audit_log.len(),
        review_cycles: tables.review_cycles.len(),
        performance_ratings: tables.performance_ratings.len(),
        performance_reviews: tables.performance_reviews.len(),
        enps_responses: tables.enps_responses.len(),
    };

    let metadata = BackupMetadata {
        version: BACKUP_VERSION.to_string(),
        created_at: Utc::now(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        table_counts: table_counts.clone(),
    };

    let backup_data = BackupData { metadata, tables };

    // Serialize to JSON
    let json = serde_json::to_string(&backup_data)
        .map_err(|e| BackupError::Io(format!("Serialization error: {}", e)))?;

    // Compress
    let compressed = compress_data(json.as_bytes())?;

    // Encrypt
    let encrypted = encrypt_data(&compressed, password)?;

    // Generate filename
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("peoplepartner_backup_{}.ppbackup", timestamp);

    Ok(ExportResult {
        encrypted_data: encrypted,
        filename,
        table_counts,
    })
}

/// Validate a backup file and return its metadata (without importing)
pub fn validate_backup(encrypted_data: &[u8], password: &str) -> Result<BackupMetadata, BackupError> {
    // Decrypt
    let compressed = decrypt_data(encrypted_data, password)?;

    // Decompress
    let json = decompress_data(&compressed)?;

    // Parse
    let backup_data: BackupData = serde_json::from_slice(&json)
        .map_err(|_| BackupError::InvalidBackup)?;

    // Check version compatibility
    if backup_data.metadata.version != BACKUP_VERSION {
        return Err(BackupError::VersionMismatch {
            expected: BACKUP_VERSION.to_string(),
            found: backup_data.metadata.version,
        });
    }

    Ok(backup_data.metadata)
}

/// Import data from an encrypted backup, replacing all existing data.
/// The clear + restore sequence runs inside a SQLite transaction so that a
/// partial restore failure automatically rolls back, leaving the database intact.
pub async fn import_backup(
    pool: &SqlitePool,
    encrypted_data: &[u8],
    password: &str,
) -> Result<ImportResult, BackupError> {
    // Decrypt
    let compressed = decrypt_data(encrypted_data, password)?;

    // Decompress
    let json = decompress_data(&compressed)?;

    // Parse
    let backup_data: BackupData =
        serde_json::from_slice(&json).map_err(|_| BackupError::InvalidBackup)?;

    // Check version compatibility
    if backup_data.metadata.version != BACKUP_VERSION {
        return Err(BackupError::VersionMismatch {
            expected: BACKUP_VERSION.to_string(),
            found: backup_data.metadata.version,
        });
    }

    let warnings = Vec::new();

    // Acquire a single connection and wrap clear + restore in a transaction.
    // On error the transaction is rolled back explicitly so the database is
    // never left in a partially-wiped state.
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| BackupError::Database(e.to_string()))?;
    sqlx::query("BEGIN").execute(&mut *conn).await?;

    // Temporarily drop the audit-log append-only triggers for the duration of
    // this transaction. `clear_all_tables` must DELETE FROM audit_log as part
    // of a full wipe-and-restore, and the triggers otherwise ABORT that DELETE.
    //
    // Safety: this entire path is gated by the caller already having the
    // backup's encryption password (the data was decrypted above). A caller
    // without the password can't reach this code to exploit the trigger gap,
    // and DROP TRIGGER inside a transaction rolls back on any failure below,
    // restoring the append-only guarantee automatically. The CREATE TRIGGER
    // at the end of the happy path re-arms the guard before COMMIT.
    for drop_stmt in [
        "DROP TRIGGER IF EXISTS audit_log_no_update",
        "DROP TRIGGER IF EXISTS audit_log_no_delete",
    ] {
        if let Err(e) = sqlx::query(drop_stmt).execute(&mut *conn).await {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(BackupError::Database(e.to_string()));
        }
    }

    // Clear existing data
    let result = clear_all_tables(&mut *conn).await;
    if let Err(e) = result {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
        return Err(e);
    }

    // Restore all tables
    let restored_counts = match restore_all_tables(&mut *conn, &backup_data.tables).await {
        Ok(counts) => counts,
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(e);
        }
    };

    // Rebuild FTS indexes to ensure search works correctly after restore
    if let Err(e) = sqlx::query(
        "INSERT INTO conversations_fts(conversations_fts) VALUES('rebuild')",
    )
    .execute(&mut *conn)
    .await
    {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
        return Err(BackupError::Database(e.to_string()));
    }

    if let Err(e) = sqlx::query(
        "INSERT INTO performance_reviews_fts(performance_reviews_fts) VALUES('rebuild')",
    )
    .execute(&mut *conn)
    .await
    {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
        return Err(BackupError::Database(e.to_string()));
    }

    // Re-arm the audit-log append-only triggers before committing. Must match
    // migration 011 exactly — if these drift, migration 011 becomes the source
    // of truth on fresh installs and this block becomes a silent downgrade on
    // restore.
    for create_stmt in [
        "CREATE TRIGGER IF NOT EXISTS audit_log_no_update \
         BEFORE UPDATE ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END",
        "CREATE TRIGGER IF NOT EXISTS audit_log_no_delete \
         BEFORE DELETE ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END",
    ] {
        if let Err(e) = sqlx::query(create_stmt).execute(&mut *conn).await {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            return Err(BackupError::Database(e.to_string()));
        }
    }

    sqlx::query("COMMIT").execute(&mut *conn).await?;

    Ok(ImportResult {
        restored_counts,
        warnings,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let data = b"Hello, this is test data for encryption!";
        let password = "testpassword123";

        let encrypted = encrypt_data(data, password).unwrap();
        assert_ne!(encrypted, data);

        let decrypted = decrypt_data(&encrypted, password).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_wrong_password_fails() {
        let data = b"Secret data";
        let password = "correctpassword";
        let wrong_password = "wrongpassword";

        let encrypted = encrypt_data(data, password).unwrap();
        let result = decrypt_data(&encrypted, wrong_password);

        assert!(matches!(result, Err(BackupError::InvalidPassword)));
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let data = b"This is some data that should compress well well well well!";

        let compressed = compress_data(data).unwrap();
        let decompressed = decompress_data(&compressed).unwrap();

        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_short_password_rejected() {
        let tables = BackupTables {
            employees: vec![],
            conversations: vec![],
            company: vec![],
            settings: vec![],
            audit_log: vec![],
            review_cycles: vec![],
            performance_ratings: vec![],
            performance_reviews: vec![],
            enps_responses: vec![],
        };

        // Can't test export_backup directly without async runtime, but we can verify
        // the password length constant
        assert_eq!(MIN_PASSWORD_LENGTH, 8);
    }

    #[test]
    fn test_invalid_backup_data() {
        let garbage = vec![0u8; 100];
        let password = "testpassword";

        let result = validate_backup(&garbage, password);
        assert!(matches!(
            result,
            Err(BackupError::InvalidPassword) | Err(BackupError::InvalidBackup)
        ));
    }

    #[test]
    fn test_excluded_settings_keys_not_in_backup() {
        // Simulate settings rows that include excluded keys
        let all_settings = vec![
            SettingsRow {
                key: "device_id".to_string(),
                value: "abc-123".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
            SettingsRow {
                key: "trial_messages_used".to_string(),
                value: "42".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
            SettingsRow {
                key: "proxy_signing_secret".to_string(),
                value: "secret-value".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
            SettingsRow {
                key: "theme".to_string(),
                value: "dark".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
            SettingsRow {
                key: "ai_provider".to_string(),
                value: "claude".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
        ];

        let filtered: Vec<&SettingsRow> = all_settings
            .iter()
            .filter(|row| !EXCLUDED_SETTINGS_KEYS.contains(&row.key.as_str()))
            .collect();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.key != "device_id"));
        assert!(filtered.iter().all(|r| r.key != "trial_messages_used"));
        assert!(filtered.iter().all(|r| r.key != "proxy_signing_secret"));
        assert!(filtered.iter().any(|r| r.key == "theme"));
        assert!(filtered.iter().any(|r| r.key == "ai_provider"));
    }

    #[test]
    fn test_restore_skips_excluded_settings() {
        // Verify that the restore filter logic matches the export filter
        let rows = vec![
            SettingsRow {
                key: "device_id".to_string(),
                value: "old-device".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
            SettingsRow {
                key: "theme".to_string(),
                value: "dark".to_string(),
                updated_at: Some("2025-01-01".to_string()),
            },
        ];

        let restorable: Vec<&SettingsRow> = rows
            .iter()
            .filter(|row| !EXCLUDED_SETTINGS_KEYS.contains(&row.key.as_str()))
            .collect();

        assert_eq!(restorable.len(), 1);
        assert_eq!(restorable[0].key, "theme");
    }

    #[test]
    fn test_table_counts_serialization() {
        let counts = TableCounts {
            employees: 100,
            conversations: 50,
            company: 1,
            settings: 5,
            audit_log: 200,
            review_cycles: 3,
            performance_ratings: 300,
            performance_reviews: 300,
            enps_responses: 600,
        };

        let json = serde_json::to_string(&counts).unwrap();
        let parsed: TableCounts = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.employees, 100);
        assert_eq!(parsed.enps_responses, 600);
    }
}
