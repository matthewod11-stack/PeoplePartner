#!/usr/bin/env npx tsx
/**
 * Manual extraction script - runs highlights extraction on all pending reviews
 * Usage: npx tsx scripts/run-extraction.ts
 */

import { readFileSync } from 'fs';
import { homedir } from 'os';
import { join } from 'path';
import Database from 'better-sqlite3';

const APP_DATA_DIR = join(homedir(), 'Library/Application Support/com.peoplepartner.app');
const DB_PATH = join(APP_DATA_DIR, 'people_partner.db');
const API_KEY_PATH = join(APP_DATA_DIR, '.api_key');

// Valid themes (must match Rust validation)
const VALID_THEMES = [
  'leadership', 'technical-growth', 'communication', 'collaboration',
  'execution', 'learning', 'innovation', 'mentoring', 'problem-solving', 'customer-focus'
];

interface Review {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  strengths: string | null;
  areas_for_improvement: string | null;
  accomplishments: string | null;
  manager_comments: string | null;
}

interface ExtractionResult {
  strengths: string[];
  opportunities: string[];
  themes: string[];
  quotes: { sentiment: string; text: string }[];
  overall_sentiment: string;
}

async function extractHighlights(apiKey: string, review: Review): Promise<ExtractionResult> {
  const reviewText = [
    review.strengths && `Strengths: ${review.strengths}`,
    review.areas_for_improvement && `Areas for Improvement: ${review.areas_for_improvement}`,
    review.accomplishments && `Accomplishments: ${review.accomplishments}`,
    review.manager_comments && `Manager Comments: ${review.manager_comments}`,
  ].filter(Boolean).join('\n\n');

  const systemPrompt = `You are an HR data extraction assistant. Extract structured information from performance review text.

Return ONLY valid JSON with this exact structure:
{
  "strengths": ["strength1", "strength2"],
  "opportunities": ["area1", "area2"],
  "themes": ["theme1", "theme2"],
  "quotes": [{"sentiment": "positive", "text": "quote text"}],
  "overall_sentiment": "positive|neutral|mixed|negative"
}

Rules:
- strengths: 2-5 key strengths mentioned
- opportunities: 2-5 development areas or growth opportunities
- themes: 1-3 themes from ONLY these options: ${VALID_THEMES.join(', ')}
- quotes: 1-3 notable quotes with sentiment (positive/negative/neutral)
- overall_sentiment: one of positive, neutral, mixed, negative

Return ONLY the JSON, no markdown fences or explanation.`;

  const response = await fetch('https://api.anthropic.com/v1/messages', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-api-key': apiKey,
      'anthropic-version': '2023-06-01',
    },
    body: JSON.stringify({
      model: 'claude-3-haiku-20240307',
      max_tokens: 1024,
      system: systemPrompt,
      messages: [{ role: 'user', content: reviewText }],
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`API error: ${response.status} - ${error}`);
  }

  const data = await response.json();
  const content = data.content[0]?.text || '{}';

  // Strip markdown fences if present
  const jsonStr = content.replace(/```json\n?/g, '').replace(/```\n?/g, '').trim();

  try {
    const parsed = JSON.parse(jsonStr);
    // Validate and filter themes
    parsed.themes = (parsed.themes || []).filter((t: string) => VALID_THEMES.includes(t));
    return parsed;
  } catch {
    console.error('Failed to parse JSON:', jsonStr);
    return {
      strengths: [],
      opportunities: [],
      themes: [],
      quotes: [],
      overall_sentiment: 'neutral',
    };
  }
}

async function main() {
  console.log('=== People Partner - Manual Extraction ===\n');

  // Load API key
  let apiKey: string;
  try {
    apiKey = readFileSync(API_KEY_PATH, 'utf-8').trim();
    console.log(`API key loaded (${apiKey.slice(0, 10)}...)`);
  } catch {
    console.error('ERROR: Could not read API key from', API_KEY_PATH);
    process.exit(1);
  }

  // Open database
  const db = new Database(DB_PATH);
  console.log(`Database opened: ${DB_PATH}\n`);

  // Find reviews without highlights
  const pendingReviews = db.prepare(`
    SELECT pr.id, pr.employee_id, pr.review_cycle_id,
           pr.strengths, pr.areas_for_improvement, pr.accomplishments, pr.manager_comments
    FROM performance_reviews pr
    LEFT JOIN review_highlights rh ON pr.id = rh.review_id
    WHERE rh.id IS NULL
  `).all() as Review[];

  console.log(`Found ${pendingReviews.length} reviews pending extraction\n`);

  if (pendingReviews.length === 0) {
    console.log('Nothing to do!');
    db.close();
    return;
  }

  // Prepare insert statement
  const insertHighlight = db.prepare(`
    INSERT INTO review_highlights (id, review_id, employee_id, review_cycle_id,
                                   strengths, opportunities, themes, quotes,
                                   overall_sentiment, extraction_model, extraction_version)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'claude-3-haiku-20240307', 1)
  `);

  let successCount = 0;
  let errorCount = 0;

  for (let i = 0; i < pendingReviews.length; i++) {
    const review = pendingReviews[i];
    const progress = `[${i + 1}/${pendingReviews.length}]`;

    try {
      process.stdout.write(`${progress} Extracting ${review.id.slice(0, 12)}... `);

      const result = await extractHighlights(apiKey, review);

      const highlightId = `hl_${crypto.randomUUID().replace(/-/g, '').slice(0, 8)}`;

      insertHighlight.run(
        highlightId,
        review.id,
        review.employee_id,
        review.review_cycle_id,
        JSON.stringify(result.strengths),
        JSON.stringify(result.opportunities),
        JSON.stringify(result.themes),
        JSON.stringify(result.quotes),
        result.overall_sentiment
      );

      console.log(`OK (themes: ${result.themes.join(', ') || 'none'})`);
      successCount++;

      // Rate limiting - 100ms delay between calls
      await new Promise(resolve => setTimeout(resolve, 100));

    } catch (error) {
      console.log(`ERROR: ${error}`);
      errorCount++;
    }
  }

  console.log(`\n=== Extraction Complete ===`);
  console.log(`Success: ${successCount}`);
  console.log(`Errors: ${errorCount}`);

  // Now generate employee summaries
  console.log(`\n=== Generating Employee Summaries ===\n`);

  const employeesWithHighlights = db.prepare(`
    SELECT DISTINCT employee_id FROM review_highlights
  `).all() as { employee_id: string }[];

  console.log(`Found ${employeesWithHighlights.length} employees with highlights\n`);

  const insertSummary = db.prepare(`
    INSERT OR REPLACE INTO employee_summaries
    (id, employee_id, career_narrative, key_strengths, development_areas,
     notable_accomplishments, reviews_analyzed, generation_model)
    VALUES (?, ?, ?, ?, ?, ?, ?, 'claude-3-haiku-20240307')
  `);

  for (let i = 0; i < employeesWithHighlights.length; i++) {
    const { employee_id } = employeesWithHighlights[i];
    const progress = `[${i + 1}/${employeesWithHighlights.length}]`;

    try {
      process.stdout.write(`${progress} Summarizing ${employee_id.slice(0, 12)}... `);

      // Get all highlights for this employee
      const highlights = db.prepare(`
        SELECT strengths, opportunities, themes, quotes, overall_sentiment
        FROM review_highlights WHERE employee_id = ?
      `).all(employee_id) as any[];

      // Aggregate data
      const allStrengths = highlights.flatMap(h => JSON.parse(h.strengths));
      const allOpportunities = highlights.flatMap(h => JSON.parse(h.opportunities));
      const allQuotes = highlights.flatMap(h => JSON.parse(h.quotes));

      // Generate narrative via Claude
      const summaryPrompt = `Based on ${highlights.length} performance reviews, create a brief 2-3 sentence career narrative for this employee.

Strengths mentioned: ${[...new Set(allStrengths)].join(', ')}
Development areas: ${[...new Set(allOpportunities)].join(', ')}
Sample quotes: ${allQuotes.slice(0, 3).map((q: any) => q.text).join('; ')}

Return ONLY a JSON object:
{
  "narrative": "2-3 sentence career summary",
  "key_strengths": ["top 3 strengths"],
  "development_areas": ["top 2-3 areas"],
  "accomplishments": ["1-2 notable accomplishments if any"]
}`;

      const response = await fetch('https://api.anthropic.com/v1/messages', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'x-api-key': apiKey,
          'anthropic-version': '2023-06-01',
        },
        body: JSON.stringify({
          model: 'claude-3-haiku-20240307',
          max_tokens: 512,
          messages: [{ role: 'user', content: summaryPrompt }],
        }),
      });

      if (!response.ok) throw new Error(`API ${response.status}`);

      const data = await response.json();
      const content = data.content[0]?.text || '{}';
      const jsonStr = content.replace(/```json\n?/g, '').replace(/```\n?/g, '').trim();
      const summary = JSON.parse(jsonStr);

      const summaryId = `sum_${crypto.randomUUID().replace(/-/g, '').slice(0, 8)}`;

      insertSummary.run(
        summaryId,
        employee_id,
        summary.narrative || null,
        JSON.stringify(summary.key_strengths || []),
        JSON.stringify(summary.development_areas || []),
        JSON.stringify(summary.accomplishments || []),
        highlights.length
      );

      console.log('OK');

      await new Promise(resolve => setTimeout(resolve, 100));

    } catch (error) {
      console.log(`ERROR: ${error}`);
    }
  }

  db.close();
  console.log('\n=== Done! ===');
}

main().catch(console.error);
