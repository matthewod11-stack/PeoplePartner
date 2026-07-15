import { describe, it, expect } from 'vitest';
import { formatDate, calculateTenure, parseLocalDate } from './utils';

describe('parseLocalDate', () => {
  it('parses a date-only string to LOCAL midnight (not UTC midnight)', () => {
    // Regression for #151: new Date('2023-05-15') is UTC midnight, which is the
    // previous calendar day in any negative-offset timezone. parseLocalDate must
    // land on the same calendar day the string names, regardless of the runner TZ.
    const d = parseLocalDate('2023-05-15');
    expect(d.getFullYear()).toBe(2023);
    expect(d.getMonth()).toBe(4); // May (0-indexed)
    expect(d.getDate()).toBe(15);
  });

  it('passes strings that carry a time component through to the native parser', () => {
    const d = parseLocalDate('2023-05-15T12:00:00Z');
    expect(Number.isNaN(d.getTime())).toBe(false);
  });
});

describe('formatDate', () => {
  it('renders a date-only string on its own calendar day regardless of local timezone', () => {
    // #151: hire_date "2023-05-15" was rendering as "May 14, 2023" in PDT.
    expect(formatDate('2023-05-15')).toBe('May 15, 2023');
  });

  it('formats another date-only value on the correct day', () => {
    expect(formatDate('2024-01-15')).toBe('Jan 15, 2024');
  });

  it('does not drift across a year boundary', () => {
    expect(formatDate('2024-01-01')).toBe('Jan 1, 2024');
  });

  it('returns an em dash for missing input', () => {
    expect(formatDate(undefined)).toBe('—');
    expect(formatDate('')).toBe('—');
  });
});

describe('calculateTenure', () => {
  it('returns an em dash for a missing hire date', () => {
    expect(calculateTenure(undefined)).toBe('—');
  });

  it('reads a multi-year date-only hire date without drifting the year down a day', () => {
    // A hire date ~3.5 years before "now" (built from local parts) should read 3y.
    const now = new Date();
    const past = new Date(now.getFullYear() - 3, now.getMonth() - 6, now.getDate());
    const y = past.getFullYear();
    const m = String(past.getMonth() + 1).padStart(2, '0');
    const d = String(past.getDate()).padStart(2, '0');
    expect(calculateTenure(`${y}-${m}-${d}`).startsWith('3y')).toBe(true);
  });
});
