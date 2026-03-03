// People Partner - Test Data Import Script
// Imports generated test data directly into SQLite
// Run with: npx tsx scripts/import-test-data.ts

import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import Database from 'better-sqlite3';
import { homedir } from 'os';

const currentDir = dirname(fileURLToPath(import.meta.url));

// Database path (macOS app support)
const DB_PATH = join(
  homedir(),
  'Library/Application Support/com.peoplepartner.app/people_partner.db'
);

// Generated data paths
const GENERATED_DIR = join(currentDir, 'generated');

interface ImportEmployee {
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

interface ImportReviewCycle {
  id: string;
  name: string;
  cycle_type: string;
  start_date: string;
  end_date: string;
  status: string;
}

interface ImportRating {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  reviewer_id?: string | null;
  overall_rating: number;
  goals_rating?: number | null;
  competency_rating?: number | null;
  submitted_at?: string | null;
}

interface ImportReview {
  id: string;
  employee_id: string;
  review_cycle_id: string;
  reviewer_id?: string | null;
  strengths?: string | null;
  areas_for_improvement?: string | null;
  accomplishments?: string | null;
  manager_comments?: string | null;
  submitted_at?: string | null;
}

interface ImportEnps {
  id: string;
  employee_id: string;
  survey_date: string;
  survey_name: string;
  score: number;
  feedback_text?: string | null;
  submitted_at?: string | null;
}

function loadJson<T>(filename: string): T {
  const path = join(GENERATED_DIR, filename);
  return JSON.parse(readFileSync(path, 'utf-8'));
}

async function main() {
  console.log('People Partner - Test Data Import\n');
  console.log('Database: ' + DB_PATH);
  console.log('Generated data: ' + GENERATED_DIR + '\n');

  // Open database
  const db = new Database(DB_PATH);

  try {
    // Load generated data
    console.log('Loading generated data...');
    const employees = loadJson<ImportEmployee[]>('employees.json');
    const reviewCycles = loadJson<ImportReviewCycle[]>('review-cycles.json');
    const ratings = loadJson<ImportRating[]>('ratings.json');
    const reviews = loadJson<ImportReview[]>('reviews.json');
    const enps = loadJson<ImportEnps[]>('enps.json');

    console.log('  Employees: ' + employees.length);
    console.log('  Review Cycles: ' + reviewCycles.length);
    console.log('  Ratings: ' + ratings.length);
    console.log('  Reviews: ' + reviews.length);
    console.log('  eNPS: ' + enps.length + '\n');

    // Clear existing data
    console.log('Clearing existing data...');
    db.prepare('DELETE FROM enps_responses').run();
    db.prepare('DELETE FROM performance_reviews').run();
    db.prepare('DELETE FROM performance_ratings').run();
    db.prepare('DELETE FROM employees').run();
    db.prepare('DELETE FROM review_cycles').run();
    console.log('  Done\n');

    // Import review cycles first (no FKs)
    console.log('Importing review cycles...');
    const insertCycle = db.prepare(
      'INSERT INTO review_cycles (id, name, cycle_type, start_date, end_date, status) VALUES (?, ?, ?, ?, ?, ?)'
    );
    for (const cycle of reviewCycles) {
      insertCycle.run(cycle.id, cycle.name, cycle.cycle_type, cycle.start_date, cycle.end_date, cycle.status);
    }
    console.log('  Inserted ' + reviewCycles.length + ' cycles\n');

    // Import employees
    console.log('Importing employees...');
    const insertEmployee = db.prepare(
      'INSERT INTO employees (id, email, full_name, department, job_title, manager_id, hire_date, work_state, status, date_of_birth, gender, ethnicity, termination_date, termination_reason) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)'
    );
    for (const emp of employees) {
      insertEmployee.run(
        emp.id, emp.email, emp.full_name, emp.department, emp.job_title, emp.manager_id,
        emp.hire_date, emp.work_state, emp.status || 'active', emp.date_of_birth,
        emp.gender, emp.ethnicity, emp.termination_date, emp.termination_reason
      );
    }
    console.log('  Inserted ' + employees.length + ' employees\n');

    // Import ratings
    console.log('Importing performance ratings...');
    const insertRating = db.prepare(
      'INSERT INTO performance_ratings (id, employee_id, review_cycle_id, reviewer_id, overall_rating, goals_rating, competencies_rating, rating_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?)'
    );
    for (const rating of ratings) {
      insertRating.run(
        rating.id, rating.employee_id, rating.review_cycle_id, rating.reviewer_id,
        rating.overall_rating, rating.goals_rating, rating.competency_rating, rating.submitted_at
      );
    }
    console.log('  Inserted ' + ratings.length + ' ratings\n');

    // Import reviews
    console.log('Importing performance reviews...');
    const insertReview = db.prepare(
      'INSERT INTO performance_reviews (id, employee_id, review_cycle_id, reviewer_id, strengths, areas_for_improvement, accomplishments, manager_comments, review_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)'
    );
    for (const review of reviews) {
      insertReview.run(
        review.id, review.employee_id, review.review_cycle_id, review.reviewer_id,
        review.strengths, review.areas_for_improvement, review.accomplishments,
        review.manager_comments, review.submitted_at
      );
    }
    console.log('  Inserted ' + reviews.length + ' reviews\n');

    // Import eNPS
    console.log('Importing eNPS responses...');
    const insertEnps = db.prepare(
      'INSERT INTO enps_responses (id, employee_id, survey_date, survey_name, score, feedback_text) VALUES (?, ?, ?, ?, ?, ?)'
    );
    for (const response of enps) {
      insertEnps.run(
        response.id, response.employee_id, response.survey_date,
        response.survey_name, response.score, response.feedback_text
      );
    }
    console.log('  Inserted ' + enps.length + ' responses\n');

    // Verify integrity
    console.log('Verifying data integrity...\n');

    // Check counts
    const empCount = (db.prepare('SELECT COUNT(*) as c FROM employees').get() as {c: number}).c;
    const cycleCount = (db.prepare('SELECT COUNT(*) as c FROM review_cycles').get() as {c: number}).c;
    const ratingCount = (db.prepare('SELECT COUNT(*) as c FROM performance_ratings').get() as {c: number}).c;
    const reviewCount = (db.prepare('SELECT COUNT(*) as c FROM performance_reviews').get() as {c: number}).c;
    const enpsCount = (db.prepare('SELECT COUNT(*) as c FROM enps_responses').get() as {c: number}).c;

    console.log('Record counts:');
    console.log('  Employees: ' + empCount);
    console.log('  Review Cycles: ' + cycleCount);
    console.log('  Ratings: ' + ratingCount);
    console.log('  Reviews: ' + reviewCount);
    console.log('  eNPS: ' + enpsCount + '\n');

    // Check FK integrity
    const orphanRatingEmps = (db.prepare(
      'SELECT COUNT(*) as c FROM performance_ratings pr WHERE NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = pr.employee_id)'
    ).get() as {c: number}).c;

    const orphanRatingReviewers = (db.prepare(
      'SELECT COUNT(*) as c FROM performance_ratings pr WHERE pr.reviewer_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = pr.reviewer_id)'
    ).get() as {c: number}).c;

    const orphanRatingCycles = (db.prepare(
      'SELECT COUNT(*) as c FROM performance_ratings pr WHERE NOT EXISTS (SELECT 1 FROM review_cycles rc WHERE rc.id = pr.review_cycle_id)'
    ).get() as {c: number}).c;

    const orphanEnps = (db.prepare(
      'SELECT COUNT(*) as c FROM enps_responses er WHERE NOT EXISTS (SELECT 1 FROM employees e WHERE e.id = er.employee_id)'
    ).get() as {c: number}).c;

    const orphanManagers = (db.prepare(
      'SELECT COUNT(*) as c FROM employees e WHERE e.manager_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM employees m WHERE m.id = e.manager_id)'
    ).get() as {c: number}).c;

    console.log('Integrity checks:');
    console.log('  Orphan rating employee_ids: ' + orphanRatingEmps + ' ' + (orphanRatingEmps === 0 ? 'PASS' : 'FAIL'));
    console.log('  Orphan rating reviewer_ids: ' + orphanRatingReviewers + ' ' + (orphanRatingReviewers === 0 ? 'PASS' : 'FAIL'));
    console.log('  Orphan rating cycle_ids: ' + orphanRatingCycles + ' ' + (orphanRatingCycles === 0 ? 'PASS' : 'FAIL'));
    console.log('  Orphan eNPS employee_ids: ' + orphanEnps + ' ' + (orphanEnps === 0 ? 'PASS' : 'FAIL'));
    console.log('  Orphan manager_ids: ' + orphanManagers + ' ' + (orphanManagers === 0 ? 'PASS' : 'FAIL') + '\n');

    // Run verification queries from plan
    console.log('Verification queries:\n');

    // Who has been here longest?
    const longestTenure = db.prepare(
      "SELECT full_name, hire_date, CAST((julianday('now') - julianday(hire_date)) / 365 AS INTEGER) as years FROM employees WHERE status = 'active' ORDER BY hire_date ASC LIMIT 1"
    ).get() as { full_name: string; hire_date: string; years: number };
    console.log('Q: "Who has been here longest?"');
    console.log('A: ' + longestTenure.full_name + ' (' + longestTenure.years + ' years, hired ' + longestTenure.hire_date + ')\n');

    // Who is underperforming?
    const underperforming = db.prepare(
      'SELECT e.full_name, AVG(pr.overall_rating) as avg_rating, COUNT(*) as review_count FROM employees e JOIN performance_ratings pr ON e.id = pr.employee_id GROUP BY e.id HAVING avg_rating < 2.5 AND review_count >= 2 ORDER BY avg_rating ASC'
    ).all() as { full_name: string; avg_rating: number; review_count: number }[];
    console.log('Q: "Who is underperforming?" (avg < 2.5 over 2+ cycles)');
    for (const emp of underperforming) {
      console.log('A: ' + emp.full_name + ' (avg: ' + emp.avg_rating.toFixed(2) + ', ' + emp.review_count + ' reviews)');
    }
    if (underperforming.length === 0) {
      console.log('A: No employees match criteria');
    }
    console.log();

    // What is our eNPS?
    const latestSurvey = db.prepare(
      "SELECT survey_name, COUNT(CASE WHEN score >= 9 THEN 1 END) as promoters, COUNT(CASE WHEN score <= 6 THEN 1 END) as detractors, COUNT(*) as total FROM enps_responses WHERE survey_name = (SELECT survey_name FROM enps_responses ORDER BY survey_date DESC LIMIT 1) GROUP BY survey_name"
    ).get() as { survey_name: string; promoters: number; detractors: number; total: number };
    const enpsScore = Math.round(((latestSurvey.promoters - latestSurvey.detractors) / latestSurvey.total) * 100);
    console.log('Q: "What is our eNPS?" (' + latestSurvey.survey_name + ')');
    console.log('A: eNPS: ' + (enpsScore >= 0 ? '+' : '') + enpsScore);
    console.log('   Promoters: ' + latestSurvey.promoters + ', Detractors: ' + latestSurvey.detractors + ', Total: ' + latestSurvey.total + '\n');

    // Check Amanda Foster
    const amandaFoster = db.prepare(
      "SELECT e.full_name, e.status, e.termination_date, (SELECT COUNT(*) FROM performance_ratings pr WHERE pr.employee_id = e.id AND pr.review_cycle_id = 'rc_2025_q1') as q1_2025_ratings FROM employees e WHERE e.full_name LIKE '%Amanda%Foster%' OR e.email LIKE '%amanda%foster%'"
    ).get() as { full_name: string; status: string; termination_date: string; q1_2025_ratings: number } | undefined;

    console.log('Q: "Amanda Foster Q1 2025 data check"');
    if (amandaFoster) {
      console.log('A: ' + amandaFoster.full_name + ': ' + amandaFoster.status);
      console.log('   Terminated: ' + amandaFoster.termination_date);
      console.log('   Q1 2025 ratings: ' + amandaFoster.q1_2025_ratings + ' ' + (amandaFoster.q1_2025_ratings === 0 ? 'PASS' : 'FAIL'));
    } else {
      console.log('A: Not found in database');
    }
    console.log();

    console.log('Import complete!');

  } finally {
    db.close();
  }
}

main().catch(console.error);
