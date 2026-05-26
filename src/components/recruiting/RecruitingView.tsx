// People Partner — Recruit module (talent sourcing)
//
// S0.1 skeleton (FHR-70): an empty Recruiting view, rendered in the main
// content area when the `recruiting` tab is active and gated behind
// RECRUITING_ENABLED. This is the first visible seam for the module — S0.2
// stands up the Rust `recruiting` module + first command, and S0.3 round-trips
// a live Exa search into this view. Intentionally empty until then.

export function RecruitingView() {
  return (
    <div className="h-full flex flex-col items-center justify-center text-center px-6">
      <div className="w-14 h-14 rounded-full bg-primary-100 flex items-center justify-center mb-4">
        <svg
          className="w-7 h-7 text-primary-600"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={1.5}
          aria-hidden="true"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
        </svg>
      </div>
      <h2 className="text-lg font-display font-semibold text-stone-800 mb-1">Recruit</h2>
      <p className="text-sm text-stone-500 max-w-sm">
        Context-aware talent sourcing, seeded by the employee data you already
        have. This module is under construction — nothing to show here yet.
      </p>
    </div>
  );
}

export default RecruitingView;
