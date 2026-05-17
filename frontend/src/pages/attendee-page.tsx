import { RouteCard } from '@/components/route-card';

export function AttendeePage() {
  return (
    <RouteCard
      eyebrow="Attendee"
      title="Attendee area placeholder"
      description="Discovery, registration state, and attendee dashboards will extend this route in follow-up issues."
    >
      <div className="rounded-2xl border border-dashed border-border/70 p-5 text-sm leading-6 text-muted-foreground">
        The app shell and shared providers are ready for future public event and
        registration flows.
      </div>
    </RouteCard>
  );
}
