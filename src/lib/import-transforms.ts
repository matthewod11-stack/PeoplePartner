// Pure transforms for the Data Quality Center import pipeline (#115 Floor 2).
//
// The heavy lifting — parsing, header normalization, validation, dedupe — is
// Rust (see tauri-commands + the src-tauri test suite). These are the small
// frontend-only shaping steps that were previously inline in useImportPipeline
// / ColumnMappingStep, extracted so they're unit-testable and no longer
// duplicated across the confirm-mapping and re-validate paths.

import type { ParsedRow, ColumnMapping, ValidationIssue, IssueRow } from './types';

/**
 * Group validation issues by their 1-based source row and pair each with its
 * row data (as a 0-based index into `rows`). Rows whose 0-based index falls
 * outside `rows` are skipped. Extracted verbatim from the two identical bodies
 * in useImportPipeline (confirmMapping + fixAndRevalidate).
 */
export function buildIssueRows(issues: ValidationIssue[], rows: ParsedRow[]): IssueRow[] {
  const issuesByRow = new Map<number, ValidationIssue[]>();
  for (const issue of issues) {
    const existing = issuesByRow.get(issue.row) ?? [];
    existing.push(issue);
    issuesByRow.set(issue.row, existing);
  }

  const issueRows: IssueRow[] = [];
  for (const [rowNumber, rowIssues] of issuesByRow) {
    const rowIndex = rowNumber - 1;
    if (rowIndex < rows.length) {
      issueRows.push({ rowIndex, data: rows[rowIndex], issues: rowIssues });
    }
  }
  return issueRows;
}

/**
 * Drop rows that have an 'error'-severity issue, keeping rows with only
 * warnings or no issues. Pure core of useImportPipeline.skipErrors. Issue
 * rows are 1-based; row data is 0-based.
 */
export function dropErrorRows(rows: ParsedRow[], issues: ValidationIssue[]): ParsedRow[] {
  const errorRowIndices = new Set(
    issues.filter((i) => i.severity === 'error').map((i) => i.row - 1)
  );
  return rows.filter((_, i) => !errorRowIndices.has(i));
}

/**
 * Required fields that still have no source column mapped — the "can continue"
 * gate in ColumnMappingStep. An empty array means the mapping is complete.
 */
export function getUnmappedRequiredFields(
  mapping: ColumnMapping,
  requiredFields: string[]
): string[] {
  return requiredFields.filter((f) => !mapping[f]);
}
