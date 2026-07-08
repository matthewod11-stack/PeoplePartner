import { describe, it, expect } from 'vitest';
import { buildIssueRows, dropErrorRows, getUnmappedRequiredFields } from './import-transforms';
import type { ValidationIssue, ParsedRow } from './types';

const issue = (row: number, over: Partial<ValidationIssue> = {}): ValidationIssue => ({
  row,
  column: 'email',
  value: 'bad',
  message: 'Invalid email',
  severity: 'error',
  errorType: 'invalid_email',
  ...over,
});

const rows: ParsedRow[] = [
  { name: 'A' }, // source row 1
  { name: 'B' }, // source row 2
  { name: 'C' }, // source row 3
];

describe('buildIssueRows', () => {
  it('groups multiple issues on the same row and maps 1-based → 0-based', () => {
    const result = buildIssueRows(
      [issue(2, { column: 'email' }), issue(2, { column: 'phone' })],
      rows
    );
    expect(result).toHaveLength(1);
    expect(result[0].rowIndex).toBe(1);
    expect(result[0].data).toEqual({ name: 'B' });
    expect(result[0].issues).toHaveLength(2);
  });

  it('preserves distinct rows', () => {
    const result = buildIssueRows([issue(1), issue(3)], rows);
    expect(result.map((r) => r.rowIndex).sort()).toEqual([0, 2]);
  });

  it('skips issues whose row index is past the data (out-of-range guard)', () => {
    // source row 9 → index 8, beyond the 3-row array
    expect(buildIssueRows([issue(9)], rows)).toEqual([]);
  });

  it('empty issues → empty result', () => {
    expect(buildIssueRows([], rows)).toEqual([]);
  });
});

describe('dropErrorRows', () => {
  it('removes only rows flagged with an error severity', () => {
    // row 2 has an error → dropped; rows 1 and 3 survive
    const clean = dropErrorRows(rows, [issue(2, { severity: 'error' })]);
    expect(clean).toEqual([{ name: 'A' }, { name: 'C' }]);
  });

  it('keeps rows whose only issues are warnings', () => {
    const clean = dropErrorRows(rows, [issue(2, { severity: 'warning' })]);
    expect(clean).toEqual(rows);
  });

  it('no issues → all rows kept', () => {
    expect(dropErrorRows(rows, [])).toEqual(rows);
  });

  it('a row with both a warning and an error is still dropped', () => {
    const clean = dropErrorRows(rows, [
      issue(3, { severity: 'warning' }),
      issue(3, { severity: 'error' }),
    ]);
    expect(clean).toEqual([{ name: 'A' }, { name: 'B' }]);
  });
});

describe('getUnmappedRequiredFields', () => {
  it('returns required fields with no mapped column', () => {
    expect(getUnmappedRequiredFields({ name: 'Full Name' }, ['name', 'email'])).toEqual(['email']);
  });

  it('empty when every required field is mapped', () => {
    expect(getUnmappedRequiredFields({ name: 'N', email: 'E' }, ['name', 'email'])).toEqual([]);
  });

  it('treats an empty-string mapping as unmapped', () => {
    expect(getUnmappedRequiredFields({ name: '' }, ['name'])).toEqual(['name']);
  });
});
