// Prep Brief modal + header action (FHR-110), via the mockCommands IPC
// harness (#115). Covers: generate-on-open, Facts/Threads separation with
// the inference label, thin-record note, error -> Try again, and the
// is_sample guard on the header action.
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { Employee } from '../../lib/types';
import { mockCommands } from '../../test/tauri';
import { EmployeeHeader } from '../employees/detail/EmployeeHeader';
import { PrepBriefModal } from './PrepBriefModal';

function employee(overrides: Partial<Employee> = {}): Employee {
  return {
    id: 'emp-1',
    email: 'ada@example.com',
    full_name: 'Ada Example',
    job_title: 'Software Engineer',
    department: 'Engineering',
    status: 'active',
    is_sample: false,
    created_at: '2026-01-01',
    updated_at: '2026-01-01',
    ...overrides,
  };
}

const fullBrief = {
  employeeId: 'emp-1',
  facts: [
    { text: 'Led the gateway migration with zero downtime.', citationId: 'C7' },
    { text: 'Mentored six junior engineers.', citationId: 'C9' },
  ],
  threads: [
    {
      anchorCitationId: 'C7',
      anchorFact: 'Led the gateway migration.',
      question: 'What made the cutover smooth?',
    },
  ],
};

const thinBrief = {
  employeeId: 'emp-1',
  facts: [{ text: 'Joined as HR Coordinator in December 2025.', citationId: 'C1' }],
  threads: [],
  thinRecordNote:
    'This record is too thin to anchor conversation threads — add performance-review narratives or import documents to enrich future briefs.',
};

describe('PrepBriefModal', () => {
  it('generates on open and renders Facts and labeled Threads', async () => {
    mockCommands({
      people_map_generate_brief: () => fullBrief,
    });
    render(<PrepBriefModal employee={employee()} onClose={() => {}} />);

    // Loading state appears first.
    expect(screen.getByRole('status')).toBeInTheDocument();

    // Facts section with citation chips.
    expect(
      await screen.findByText('Led the gateway migration with zero downtime.')
    ).toBeInTheDocument();
    // C7 is cited twice: once as a fact chip, once as the thread's anchor.
    expect(screen.getAllByText('C7')).toHaveLength(2);

    // Threads visibly separated under the inference label.
    expect(
      screen.getByText(/Threads to pull — prep suggestions, not facts/i)
    ).toBeInTheDocument();
    expect(
      screen.getByText('What made the cutover smooth?')
    ).toBeInTheDocument();

    // Ephemeral contract surfaced to the user.
    expect(screen.getByText(/never stored/i)).toBeInTheDocument();
  });

  it('renders the thin-record note instead of threads', async () => {
    mockCommands({
      people_map_generate_brief: () => thinBrief,
    });
    render(<PrepBriefModal employee={employee()} onClose={() => {}} />);

    expect(
      await screen.findByText(/too thin to anchor conversation threads/i)
    ).toBeInTheDocument();
    expect(screen.queryByText(/^Anchor:/)).not.toBeInTheDocument();
  });

  it('shows the error state and regenerates on Try again', async () => {
    let calls = 0;
    mockCommands({
      people_map_generate_brief: () => {
        calls += 1;
        if (calls === 1) {
          throw new Error(
            'brief cited nothing from the record (2 phantom citations) — regenerate'
          );
        }
        return fullBrief;
      },
    });
    render(<PrepBriefModal employee={employee()} onClose={() => {}} />);

    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(
      screen.getByText(/didn't ground itself in the record/i)
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /try again/i }));
    expect(
      await screen.findByText('Led the gateway migration with zero downtime.')
    ).toBeInTheDocument();
    await waitFor(() => expect(calls).toBe(2));
  });

  it('renders nothing when no employee is set', () => {
    mockCommands({});
    render(<PrepBriefModal employee={null} onClose={() => {}} />);
    expect(screen.queryByText(/Prep Brief/)).not.toBeInTheDocument();
  });
});

describe('EmployeeHeader Prep Brief action', () => {
  it('shows the action for a real employee', () => {
    render(<EmployeeHeader employee={employee()} onPrepBrief={() => {}} />);
    expect(
      screen.getByRole('button', { name: /generate prep brief/i })
    ).toBeInTheDocument();
  });

  it('hides the action for sample employees (#118)', () => {
    render(
      <EmployeeHeader
        employee={employee({ is_sample: true })}
        onPrepBrief={() => {}}
      />
    );
    expect(
      screen.queryByRole('button', { name: /generate prep brief/i })
    ).not.toBeInTheDocument();
  });
});
