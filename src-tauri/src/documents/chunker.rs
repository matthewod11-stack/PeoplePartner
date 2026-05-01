//! Text + binary parsers and chunk-splitting helpers.
//!
//! `parse_file` dispatches to the format-specific parser by extension; each
//! parser returns `Vec<RawChunk>` with section-aware boundaries when the
//! format provides them (markdown headings, docx heading styles, csv/xlsx
//! row groups, pdf form-feed page breaks).

use std::path::Path;

use super::discovery::file_type;
use super::ingest::DocumentError;

/// Maximum chunk size in characters (~1000 tokens at 4 chars/token)
const MAX_CHUNK_CHARS: usize = 4000;

/// A parsed chunk before indexing (pre-PII-redaction)
#[derive(Debug, Clone)]
pub struct RawChunk {
    pub section_title: Option<String>,
    pub content: String,
}

/// Parse a markdown file into section-aware chunks.
/// Splits on ## headings. Content before first heading gets section_title = None.
pub fn parse_markdown(content: &str) -> Vec<RawChunk> {
    let mut chunks = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();

    for line in content.lines() {
        if line.starts_with("## ") || line.starts_with("# ") {
            // Flush previous chunk
            if !current_content.trim().is_empty() {
                chunks.push(RawChunk {
                    section_title: current_title.take(),
                    content: current_content.trim().to_string(),
                });
            }
            current_title = Some(line.trim_start_matches('#').trim().to_string());
            current_content = String::new();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Flush final chunk
    if !current_content.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    // Sub-chunk any oversized sections
    split_oversized_chunks(chunks)
}

/// Parse a plain text file into paragraph-based chunks.
/// Splits on double newlines (blank lines).
pub fn parse_plaintext(content: &str) -> Vec<RawChunk> {
    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        if current.len() + trimmed.len() > MAX_CHUNK_CHARS && !current.is_empty() {
            chunks.push(RawChunk {
                section_title: None,
                content: current.trim().to_string(),
            });
            current = String::new();
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(trimmed);
    }

    if !current.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: None,
            content: current.trim().to_string(),
        });
    }

    chunks
}

/// Parse a CSV file into row-group chunks.
/// Groups ~20 rows per chunk with the header row prepended.
pub fn parse_csv(content: &str) -> Vec<RawChunk> {
    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let header = lines.remove(0);
    let mut chunks = Vec::new();

    for (i, group) in lines.chunks(20).enumerate() {
        let mut chunk_content = format!("{}\n", header);
        for row in group {
            chunk_content.push_str(row);
            chunk_content.push('\n');
        }
        chunks.push(RawChunk {
            section_title: Some(format!("Rows {}-{}", i * 20 + 1, i * 20 + group.len())),
            content: chunk_content.trim().to_string(),
        });
    }

    chunks
}

/// Hard-split a single chunk of text that has no paragraph breaks.
/// Tries sentence boundaries (". ") → newlines → spaces → hard char limit.
fn hard_split_chunk(content: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut remaining = content;

    while remaining.len() > MAX_CHUNK_CHARS {
        let window = &remaining[..MAX_CHUNK_CHARS];

        // Try sentence boundary (". ")
        let split_pos = window.rfind(". ").map(|p| p + 2)
            // Try newline
            .or_else(|| window.rfind('\n').map(|p| p + 1))
            // Try space
            .or_else(|| window.rfind(' ').map(|p| p + 1))
            // Hard split at limit
            .unwrap_or(MAX_CHUNK_CHARS);

        pieces.push(remaining[..split_pos].trim().to_string());
        remaining = &remaining[split_pos..];
    }

    if !remaining.trim().is_empty() {
        pieces.push(remaining.trim().to_string());
    }

    pieces
}

/// Split any chunk larger than MAX_CHUNK_CHARS at paragraph boundaries,
/// falling back to hard_split_chunk for single-paragraph content.
fn split_oversized_chunks(chunks: Vec<RawChunk>) -> Vec<RawChunk> {
    let mut result = Vec::new();
    for chunk in chunks {
        if chunk.content.len() <= MAX_CHUNK_CHARS {
            result.push(chunk);
        } else {
            // Split at paragraph boundaries within the oversized chunk
            let sub_chunks = parse_plaintext(&chunk.content);

            // If paragraph splitting didn't help (single paragraph), use hard split
            let needs_hard_split = sub_chunks.iter().any(|c| c.content.len() > MAX_CHUNK_CHARS);

            if needs_hard_split {
                let pieces = hard_split_chunk(&chunk.content);
                for (i, piece) in pieces.into_iter().enumerate() {
                    let title = if i == 0 {
                        chunk.section_title.clone()
                    } else {
                        chunk.section_title.as_ref().map(|t| format!("{} (cont.)", t))
                    };
                    result.push(RawChunk {
                        section_title: title,
                        content: piece,
                    });
                }
            } else {
                for (i, mut sub) in sub_chunks.into_iter().enumerate() {
                    if i == 0 {
                        sub.section_title = chunk.section_title.clone();
                    } else {
                        sub.section_title = chunk.section_title.as_ref().map(|t| format!("{} (cont.)", t));
                    }
                    result.push(sub);
                }
            }
        }
    }
    result
}

/// Best-effort extraction of a panic message from the payload returned by
/// `catch_unwind`. Panics can carry either `&'static str` or `String`.
fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Parse a PDF file. Extracts text per page, each page becomes a chunk.
///
/// The underlying `pdf-extract` crate has historically panicked on malformed PDFs
/// (encrypted object streams, malformed xref, etc.). We wrap the extraction in
/// `catch_unwind` so a poisoned file in the watched folder cannot tear down the
/// watcher thread. Note: in release builds we compile with `panic = "abort"`, so
/// the catch is effective only in debug/test builds — the bump to pdf-extract
/// 0.10 is the primary defense in release.
pub fn parse_pdf(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let file_display = path.display().to_string();

    let extract_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(&bytes)
    }));

    let text = match extract_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("PDF extraction failed: {}", e),
            });
        }
        Err(panic_payload) => {
            let panic_msg = panic_payload_to_string(&panic_payload);
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("PDF extraction panicked: {}", panic_msg),
            });
        }
    };

    // pdf-extract returns all text concatenated; split on form feeds or large gaps
    // Fall back to paragraph-based chunking with page estimation
    let chunks = if text.contains('\u{0C}') {
        // Form feed characters indicate page breaks
        text.split('\u{0C}')
            .enumerate()
            .filter(|(_, page)| !page.trim().is_empty())
            .map(|(i, page)| RawChunk {
                section_title: Some(format!("Page {}", i + 1)),
                content: page.trim().to_string(),
            })
            .collect()
    } else {
        // No page breaks detected — use paragraph chunking
        parse_plaintext(&text)
    };

    Ok(split_oversized_chunks(chunks))
}

/// Parse a .docx file. Extracts paragraph text, splits on heading styles.
///
/// Wrapped in `catch_unwind` to defend the watcher thread against malformed
/// docx files that may cause the (unmaintained) docx-rs 0.4 parser to panic.
pub fn parse_docx(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let file_display = path.display().to_string();

    let read_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        docx_rs::read_docx(&bytes)
    }));

    let doc = match read_result {
        Ok(Ok(doc)) => doc,
        Ok(Err(e)) => {
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("DOCX parsing failed: {}", e),
            });
        }
        Err(panic_payload) => {
            let panic_msg = panic_payload_to_string(&panic_payload);
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("DOCX parsing panicked: {}", panic_msg),
            });
        }
    };

    let mut chunks = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();

    for child in doc.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            // Extract text from paragraph runs
            let mut para_text = String::new();
            for child in &para.children {
                if let docx_rs::ParagraphChild::Run(run) = child {
                    for child in &run.children {
                        if let docx_rs::RunChild::Text(text) = child {
                            para_text.push_str(&text.text);
                        }
                    }
                }
            }

            let trimmed = para_text.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this paragraph has a heading style
            let is_heading = para.property.style.as_ref().map_or(false, |s| {
                s.val.to_lowercase().starts_with("heading")
            });

            if is_heading {
                // Flush current chunk
                if !current_content.trim().is_empty() {
                    chunks.push(RawChunk {
                        section_title: current_title.take(),
                        content: current_content.trim().to_string(),
                    });
                }
                current_title = Some(trimmed);
                current_content = String::new();
            } else {
                current_content.push_str(&trimmed);
                current_content.push('\n');
            }
        }
    }

    // Flush final chunk
    if !current_content.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    Ok(split_oversized_chunks(chunks))
}

/// Parse an .xlsx/.xls file. Each sheet becomes chunks of grouped rows.
pub fn parse_xlsx(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    use calamine::{open_workbook_auto, Reader, Data};

    let mut workbook = open_workbook_auto(path).map_err(|e| DocumentError::Parse {
        file: path.display().to_string(),
        message: format!("Excel parsing failed: {}", e),
    })?;

    let mut chunks = Vec::new();
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

    for sheet_name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| {
                    row.iter()
                        .map(|cell| match cell {
                            Data::String(s) => s.clone(),
                            Data::Float(f) => f.to_string(),
                            Data::Int(i) => i.to_string(),
                            Data::Bool(b) => b.to_string(),
                            _ => String::new(),
                        })
                        .collect()
                })
                .collect();

            if rows.is_empty() {
                continue;
            }

            let header = rows[0].join(",");
            let data_rows = &rows[1..];

            for (i, group) in data_rows.chunks(20).enumerate() {
                let mut content = format!("{}\n", header);
                for row in group {
                    content.push_str(&row.join(","));
                    content.push('\n');
                }
                chunks.push(RawChunk {
                    section_title: Some(format!("{} — Rows {}-{}", sheet_name, i * 20 + 1, i * 20 + group.len())),
                    content: content.trim().to_string(),
                });
            }
        }
    }

    Ok(chunks)
}

/// Dispatch to the correct parser based on file type
pub fn parse_file(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let ext = file_type(path);
    let content_result = || -> Result<String, DocumentError> {
        Ok(std::fs::read_to_string(path)?)
    };

    match ext.as_str() {
        "md" => Ok(parse_markdown(&content_result()?)),
        "txt" => Ok(parse_plaintext(&content_result()?)),
        "csv" => Ok(parse_csv(&content_result()?)),
        "pdf" => parse_pdf(path),
        "docx" => parse_docx(path),
        "xlsx" | "xls" => parse_xlsx(path),
        _ => Err(DocumentError::Parse {
            file: path.display().to_string(),
            message: format!("Unsupported file type: {}", ext),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_markdown_sections() {
        let md = "# Intro\nWelcome to the handbook.\n\n## Leave Policies\nWe offer PTO.\n\n## Benefits\nHealth insurance available.";
        let chunks = parse_markdown(md);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].section_title.as_deref(), Some("Intro"));
        assert!(chunks[0].content.contains("Welcome"));
        assert_eq!(chunks[1].section_title.as_deref(), Some("Leave Policies"));
        assert_eq!(chunks[2].section_title.as_deref(), Some("Benefits"));
    }

    #[test]
    fn test_parse_markdown_no_headings() {
        let md = "Just plain text\nwith multiple lines\nand no headings.";
        let chunks = parse_markdown(md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].section_title.is_none());
    }

    #[test]
    fn test_parse_plaintext_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = parse_plaintext(text);
        assert_eq!(chunks.len(), 1); // All fit in one chunk
        assert!(chunks[0].content.contains("First"));
        assert!(chunks[0].content.contains("Third"));
    }

    #[test]
    fn test_parse_csv_groups_rows() {
        let mut csv = String::from("Name,Department,Title\n");
        for i in 1..=45 {
            csv.push_str(&format!("Employee{},Engineering,Dev\n", i));
        }
        let chunks = parse_csv(&csv);
        assert_eq!(chunks.len(), 3); // 20 + 20 + 5
        assert_eq!(chunks[0].section_title.as_deref(), Some("Rows 1-20"));
        assert_eq!(chunks[2].section_title.as_deref(), Some("Rows 41-45"));
        assert!(chunks[0].content.starts_with("Name,Department,Title")); // Header prepended
    }

    #[test]
    fn test_split_oversized_chunks() {
        // Create a chunk larger than MAX_CHUNK_CHARS using paragraphs
        let paragraph = "This is a paragraph with enough content to test splitting behavior for documents.\n\n";
        let repeat_count = (MAX_CHUNK_CHARS / paragraph.len()) + 10;
        let big_content = paragraph.repeat(repeat_count);
        let chunks = vec![RawChunk {
            section_title: Some("Big Section".to_string()),
            content: big_content,
        }];
        let result = split_oversized_chunks(chunks);
        assert!(result.len() > 1);
        assert_eq!(result[0].section_title.as_deref(), Some("Big Section"));
    }

    #[test]
    fn test_parse_file_dispatches_by_extension() {
        let dir = TempDir::new().unwrap();

        let md_path = dir.path().join("test.md");
        fs::write(&md_path, "# Title\nContent here").unwrap();
        let chunks = parse_file(&md_path).unwrap();
        assert!(!chunks.is_empty());

        let txt_path = dir.path().join("test.txt");
        fs::write(&txt_path, "Some plain text content.").unwrap();
        let chunks = parse_file(&txt_path).unwrap();
        assert!(!chunks.is_empty());

        let csv_path = dir.path().join("test.csv");
        fs::write(&csv_path, "Name,Role\nAlice,Dev\nBob,PM").unwrap();
        let chunks = parse_file(&csv_path).unwrap();
        assert!(!chunks.is_empty());
    }

    // Regression coverage for #29 — malformed binary input must return an
    // `Err` instead of panicking, so the watcher thread survives a poisoned
    // file dropped into the watched folder.
    #[test]
    fn test_parse_pdf_malformed_returns_error() {
        let dir = TempDir::new().unwrap();
        let pdf_path = dir.path().join("not-really.pdf");
        // Random bytes masquerading as a PDF — no valid header, xref, or objects.
        fs::write(&pdf_path, b"\x00\x01\x02not a pdf at all\xff\xfe").unwrap();

        let result = parse_pdf(&pdf_path);
        assert!(result.is_err(), "malformed PDF must surface as Err, not panic");
        if let Err(DocumentError::Parse { file, .. }) = result {
            assert!(file.ends_with("not-really.pdf"));
        } else {
            panic!("expected DocumentError::Parse for malformed PDF");
        }
    }

    #[test]
    fn test_parse_docx_malformed_returns_error() {
        let dir = TempDir::new().unwrap();
        let docx_path = dir.path().join("fake.docx");
        // docx is a zip container; arbitrary bytes must not make the parser panic.
        fs::write(&docx_path, b"PK\x03\x04not a real docx payload").unwrap();

        let result = parse_docx(&docx_path);
        assert!(result.is_err(), "malformed DOCX must surface as Err, not panic");
        if let Err(DocumentError::Parse { file, .. }) = result {
            assert!(file.ends_with("fake.docx"));
        } else {
            panic!("expected DocumentError::Parse for malformed DOCX");
        }
    }

    #[test]
    fn test_hard_split_chunk_at_sentence() {
        // Build a long string with sentence boundaries
        let sentence = "This is a test sentence with enough words to fill some space. ";
        let repeat_count = (MAX_CHUNK_CHARS / sentence.len()) + 5;
        let big = sentence.repeat(repeat_count);
        assert!(big.len() > MAX_CHUNK_CHARS);

        let pieces = hard_split_chunk(&big);
        assert!(pieces.len() > 1, "Should split into multiple pieces");
        for piece in &pieces {
            assert!(piece.len() <= MAX_CHUNK_CHARS, "Each piece should fit within limit");
        }
        // Verify content is preserved (no data loss)
        let rejoined: String = pieces.join(" ");
        // The original sentences should all be present
        assert!(rejoined.contains("This is a test sentence"));
    }

    #[test]
    fn test_hard_split_chunk_no_boundaries() {
        // A long string with no sentence boundaries, newlines, or spaces
        let big = "x".repeat(MAX_CHUNK_CHARS * 2 + 100);
        let pieces = hard_split_chunk(&big);
        assert!(pieces.len() >= 2, "Should still split via hard char limit");
        for piece in &pieces {
            assert!(piece.len() <= MAX_CHUNK_CHARS, "Each piece must be within limit");
        }
    }

    #[test]
    fn test_split_oversized_single_paragraph() {
        // A single oversized "paragraph" with no blank lines — only sentence boundaries
        let sentence = "This is a sentence in a very long document without paragraph breaks. ";
        let repeat_count = (MAX_CHUNK_CHARS / sentence.len()) + 5;
        let big_content = sentence.repeat(repeat_count);
        assert!(big_content.len() > MAX_CHUNK_CHARS);
        assert!(!big_content.contains("\n\n")); // No paragraph breaks

        let chunks = vec![RawChunk {
            section_title: Some("Monolith".to_string()),
            content: big_content,
        }];
        let result = split_oversized_chunks(chunks);
        assert!(result.len() > 1, "Single paragraph should be hard-split");
        assert_eq!(result[0].section_title.as_deref(), Some("Monolith"));
        for chunk in &result {
            assert!(chunk.content.len() <= MAX_CHUNK_CHARS, "All chunks must fit within limit");
        }
    }
}
