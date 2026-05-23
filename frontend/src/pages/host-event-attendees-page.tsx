import { Link, useParams } from 'react-router-dom';
import { Users } from 'lucide-react';

import { RouteCard } from '@/components/route-card';
import { Button } from '@/components/ui/button';

export function HostEventAttendeesPage() {
  const { eventId } = useParams();

  return (
    <RouteCard
      eyebrow="Host"
      title="Attendee management"
      description="Registration lists, exports, and attendee operations will attach to this event workspace."
    >
      <div className="space-y-4">
        <div className="flex items-center gap-3 rounded-2xl border border-dashed border-border/70 p-4 text-sm text-muted-foreground">
          <Users className="h-5 w-5 shrink-0" />
          Event {eventId} is ready for attendee-management features.
        </div>
        <Button asChild variant="outline">
          <Link to="/host">Back to dashboard</Link>
        </Button>
      </div>
    </RouteCard>
  );
}
