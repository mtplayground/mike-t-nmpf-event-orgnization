import { RouteCard } from '@/components/route-card';

export function HostPage() {
  return (
    <RouteCard
      eyebrow="Host"
      title="Host workspace placeholder"
      description="Event creation, dashboards, attendee export, and announcements will attach to this route group."
    >
      <div className="rounded-2xl border border-dashed border-border/70 p-5 text-sm leading-6 text-muted-foreground">
        The route exists now so host-only screens can be layered in without
        revisiting router and layout setup.
      </div>
    </RouteCard>
  );
}
