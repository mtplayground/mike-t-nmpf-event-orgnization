import { Database, Palette, Route } from 'lucide-react';

import { RouteCard } from '@/components/route-card';

const pillars = [
  {
    icon: Route,
    title: 'Routing scaffold',
    body: 'React Router is mounted with placeholder routes for overview, auth, host, and attendee experiences.',
  },
  {
    icon: Database,
    title: 'Query foundation',
    body: 'TanStack Query is available globally so feature pages can adopt server state incrementally.',
  },
  {
    icon: Palette,
    title: 'Theme + UI base',
    body: 'Tailwind tokens and shadcn-style primitives provide a stable design system baseline for upcoming screens.',
  },
];

export function HomePage() {
  return (
    <section className="grid gap-6 lg:grid-cols-[1.35fr_0.95fr]">
      <RouteCard
        eyebrow="Overview"
        title="Frontend platform pieces are now in place."
        description="This issue establishes the app shell so later feature tickets can focus on behavior instead of redoing core plumbing."
      >
        <div className="grid gap-4 md:grid-cols-3">
          {pillars.map(({ icon: Icon, title, body }) => (
            <div
              key={title}
              className="rounded-2xl border border-border/60 bg-background/80 p-4"
            >
              <Icon className="mb-4 h-5 w-5 text-primary" />
              <h3 className="text-sm font-semibold">{title}</h3>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                {body}
              </p>
            </div>
          ))}
        </div>
      </RouteCard>
      <RouteCard
        eyebrow="Roadmap"
        title="Next frontend slices"
        description="These routes are placeholders now, but their structure matches the feature sequence already planned for the project."
      >
        <ul className="space-y-3 text-sm text-muted-foreground">
          <li>
            Auth screens and session handling attach to the `/auth` route
            family.
          </li>
          <li>
            Host event creation and attendee management build out under `/host`.
          </li>
          <li>
            Registration and attendee state grow inside the `/attendee` area.
          </li>
        </ul>
      </RouteCard>
    </section>
  );
}
