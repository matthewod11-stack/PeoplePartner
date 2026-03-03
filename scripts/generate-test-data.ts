#!/usr/bin/env npx ts-node

/**
 * Test Data Generator - CLI Entry Point
 *
 * Generates realistic Acme Corp test data for People Partner.
 *
 * Usage:
 *   npx ts-node scripts/generate-test-data.ts --employees    # Session 1
 *   npx ts-node scripts/generate-test-data.ts --performance  # Session 2
 *   npx ts-node scripts/generate-test-data.ts --enps         # Session 3
 *   npx ts-node scripts/generate-test-data.ts --all          # All data
 *   npx ts-node scripts/generate-test-data.ts --clear        # Clear generated
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { EmployeeRegistry, SPECIAL_EMPLOYEE_EMAILS } from './generators/registry.js';
import { generateReviewCycles } from './generators/review-cycles.js';
import { generateEmployees, getSpecialEmployeeIds } from './generators/employees.js';
import { generatePerformanceData, getPerformanceStats } from './generators/performance.js';
import { generateEnpsData, getEnpsStats } from './generators/enps.js';

// ESM-compatible __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Output directory
const OUTPUT_DIR = path.join(__dirname, 'generated');

// Ensure output directory exists
function ensureOutputDir(): void {
  if (!fs.existsSync(OUTPUT_DIR)) {
    fs.mkdirSync(OUTPUT_DIR, { recursive: true });
  }
}

// Clear generated files
function clearGenerated(): void {
  if (fs.existsSync(OUTPUT_DIR)) {
    const files = fs.readdirSync(OUTPUT_DIR);
    for (const file of files) {
      fs.unlinkSync(path.join(OUTPUT_DIR, file));
    }
    console.log('Cleared generated files');
  }
}

// Write JSON file
function writeJson(filename: string, data: unknown): void {
  const filePath = path.join(OUTPUT_DIR, filename);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
  console.log(`Wrote ${filePath}`);
}

/**
 * Session 1: Generate employees and review cycles
 * Output: registry.json, employees.json, review-cycles.json
 */
function generateSession1(): void {
  console.log('\n=== Session 1: Employees & Review Cycles ===\n');

  const registry = new EmployeeRegistry();

  // Step 1: Generate review cycles FIRST (no dependencies)
  console.log('Step 1: Generating review cycles...');
  const cycles = generateReviewCycles(registry);
  writeJson('review-cycles.json', cycles);

  // Step 2: Generate all employees
  console.log('\nStep 2: Generating employees...');
  const employees = generateEmployees(registry);
  writeJson('employees.json', employees);

  // Step 3: Save registry for Session 2
  console.log('\nStep 3: Saving registry...');
  registry.save(OUTPUT_DIR);

  // Step 4: Output special employee IDs for reference
  console.log('\n=== Special Employee IDs ===');
  const specialIds = getSpecialEmployeeIds(registry);
  for (const [key, id] of Object.entries(specialIds)) {
    const emp = registry.getById(id);
    console.log(`${key}: ${id} (${emp?.full_name})`);
  }

  // Verification
  console.log('\n=== Verification ===');
  console.log(`Total employees: ${registry.count}`);
  console.log(`Total review cycles: ${registry.getAllCycles().length}`);

  // Department breakdown
  const deptCounts: Record<string, number> = {};
  for (const emp of registry.getAllEmployees()) {
    deptCounts[emp.department] = (deptCounts[emp.department] || 0) + 1;
  }
  console.log('\nDepartment counts:');
  for (const [dept, count] of Object.entries(deptCounts).sort((a, b) => b[1] - a[1])) {
    console.log(`  ${dept}: ${count}`);
  }

  // Status breakdown
  const statusCounts = {
    active: registry.getByStatus('active').length,
    terminated: registry.getByStatus('terminated').length,
    leave: registry.getByStatus('leave').length,
  };
  console.log('\nStatus counts:');
  console.log(`  Active: ${statusCounts.active}`);
  console.log(`  Terminated: ${statusCounts.terminated}`);
  console.log(`  On Leave: ${statusCounts.leave}`);

  console.log('\n=== Session 1 Complete ===');
  console.log('Output files:');
  console.log('  - scripts/generated/registry.json');
  console.log('  - scripts/generated/employees.json');
  console.log('  - scripts/generated/review-cycles.json');
  console.log('\nNext: Run Session 2 to generate performance data');
}

/**
 * Session 2: Generate performance data (ratings + reviews + eNPS)
 * Prerequisite: registry.json from Session 1
 * Output: ratings.json, reviews.json, enps.json
 */
function generateSession2(): void {
  console.log('\n=== Session 2: Performance Data + eNPS ===\n');

  // Load registry from Session 1
  console.log('Loading registry from Session 1...');
  const registryPath = path.join(OUTPUT_DIR, 'registry.json');

  if (!fs.existsSync(registryPath)) {
    console.error('ERROR: registry.json not found. Run Session 1 first (--session1)');
    process.exit(1);
  }

  const registry = EmployeeRegistry.load(OUTPUT_DIR);
  console.log(`Loaded ${registry.count} employees, ${registry.getAllCycles().length} review cycles`);

  // Task 2.1.20: Generate performance ratings + reviews
  console.log('\nTask 2.1.20: Generating performance ratings + reviews...');
  const { ratings, reviews } = generatePerformanceData(registry);
  writeJson('ratings.json', ratings);
  writeJson('reviews.json', reviews);

  // Performance stats
  const perfStats = getPerformanceStats(ratings);
  console.log('\n=== Performance Data Stats ===');
  console.log(`Total ratings: ${perfStats.totalRatings}`);
  console.log(`Average rating: ${perfStats.averageRating}`);
  console.log('Rating distribution:');
  const distPct = perfStats.distributionPercent as Record<string, string>;
  for (const [band, pct] of Object.entries(distPct)) {
    console.log(`  ${band}: ${pct}`);
  }
  console.log('Ratings per cycle:');
  const byCycle = perfStats.byCycle as Record<string, number>;
  for (const [cycle, count] of Object.entries(byCycle)) {
    console.log(`  ${cycle}: ${count}`);
  }

  // Task 2.1.21: Generate eNPS responses
  console.log('\nTask 2.1.21: Generating eNPS survey responses...');
  const enpsResponses = generateEnpsData(registry);
  writeJson('enps.json', enpsResponses);

  // eNPS stats
  const enpsStats = getEnpsStats(enpsResponses, registry);
  console.log('\n=== eNPS Stats ===');
  console.log(`Total responses: ${enpsStats.totalResponses}`);
  console.log(`Average score: ${enpsStats.averageScore}`);
  console.log(`Feedback rate: ${enpsStats.feedbackRate}`);
  console.log('eNPS by survey:');
  const enpsPerSurvey = enpsStats.enpsPerSurvey as Record<string, number>;
  for (const [survey, enps] of Object.entries(enpsPerSurvey)) {
    console.log(`  ${survey}: ${enps > 0 ? '+' : ''}${enps}`);
  }

  // Special case verifications
  console.log('\n=== Special Case Verification ===');

  // Sarah Chen scores
  const sarahScores = enpsStats.sarahChenScores as Array<{ survey: string; score: number }>;
  console.log('Sarah Chen eNPS scores (should be 9 → 7 → 6):');
  for (const s of sarahScores) {
    console.log(`  ${s.survey}: ${s.score}`);
  }

  // Jennifer Walsh team average
  console.log(`Jennifer Walsh's team avg eNPS (target ~5.2): ${enpsStats.jenniferWalshTeamAvg}`);

  // Verify special employee ratings
  console.log('\nSpecial employee ratings verification:');
  const specialEmployees = [
    { key: 'SARAH_CHEN', expected: '4.5+ all cycles' },
    { key: 'MARCUS_JOHNSON', expected: '<2.5 in 2023+2024' },
    { key: 'ELENA_RODRIGUEZ', expected: '4.5+ all cycles' },
    { key: 'JAMES_PARK', expected: '~2.8, only 2024+Q1 2025' },
    { key: 'ROBERT_KIM', expected: '~3.5 steady' },
    { key: 'AMANDA_FOSTER', expected: 'no Q1 2025 rating' },
  ];

  for (const spec of specialEmployees) {
    const email = SPECIAL_EMPLOYEE_EMAILS[spec.key as keyof typeof SPECIAL_EMPLOYEE_EMAILS];
    const emp = registry.getByEmail(email);
    if (emp) {
      const empRatings = ratings.filter(r => r.employee_id === emp.id);
      const ratingsStr = empRatings.map(r => `${r.review_cycle_id.replace('rc_', '')}: ${r.overall_rating}`).join(', ');
      console.log(`  ${emp.full_name}: ${ratingsStr}`);
      console.log(`    Expected: ${spec.expected}`);
    }
  }

  console.log('\n=== Session 2 Complete ===');
  console.log('Output files:');
  console.log('  - scripts/generated/ratings.json');
  console.log('  - scripts/generated/reviews.json');
  console.log('  - scripts/generated/enps.json');
  console.log('\nAll test data generation complete!');
}

/**
 * Main entry point
 */
function main(): void {
  const args = process.argv.slice(2);

  ensureOutputDir();

  if (args.includes('--clear')) {
    clearGenerated();
    return;
  }

  if (args.includes('--employees') || args.includes('--session1')) {
    generateSession1();
    return;
  }

  if (args.includes('--performance') || args.includes('--session2')) {
    generateSession2();
    return;
  }

  if (args.includes('--enps')) {
    // eNPS is now included in Session 2, but support standalone for flexibility
    console.log('eNPS generation is included in Session 2 (--session2)');
    console.log('Running Session 2 to generate all performance + eNPS data...');
    generateSession2();
    return;
  }

  if (args.includes('--all')) {
    generateSession1();
    generateSession2();
    return;
  }

  // Default: show help
  console.log(`
People Partner - Test Data Generator

Usage:
  npx ts-node scripts/generate-test-data.ts [options]
  npm run generate-test-data -- [options]

Options:
  --employees, --session1    Generate employees + review cycles (Session 1)
  --performance, --session2  Generate ratings, reviews, eNPS (Session 2)
  --enps                     Alias for --session2 (eNPS included in Session 2)
  --all                      Generate all test data (Session 1 + 2)
  --clear                    Clear all generated files

Output:
  All generated files go to scripts/generated/

Session 1 outputs:
  - registry.json        Central ID registry (source of truth)
  - employees.json       100 employees with hierarchy
  - review-cycles.json   3 review cycles

Session 2 outputs:
  - ratings.json         ~280 performance ratings (3 cycles)
  - reviews.json         ~280 performance review narratives
  - enps.json            ~246 eNPS survey responses (3 surveys)

Special Cases Tracked:
  - Sarah Chen: High performer (4.5+), declining eNPS (9→7→6)
  - Marcus Johnson: Underperformer (<2.5 in two cycles)
  - James Park: New hire (only 2024 Annual + Q1 2025)
  - Amanda Foster: Terminated (no Q1 2025 data)
  - Jennifer Walsh's team: Low eNPS (~5.2 avg)
`);
}

main();
