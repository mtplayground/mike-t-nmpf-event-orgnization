import { CalendarPlus, ListChecks } from 'lucide-react';
import { Link } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';

export function HostPage() {
  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
          Host
        </p>
        <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
          Host workspace
        </h2>
        <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
          Create event drafts, prepare location details, and attach cover
          artwork before publishing.
        </p>
      </div>

      <div className="grid gap-5 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Create an event</CardTitle>
            <CardDescription>
              Start a host-owned draft with schedule, location, visibility, and
              optional cover image details.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button asChild>
              <Link to="/host/events/new">
                <CalendarPlus className="h-4 w-4" />
                New event
              </Link>
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Event operations</CardTitle>
            <CardDescription>
              Host list, attendee export, announcements, and registration tools
              will continue to build on the event records.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-3 rounded-2xl border border-dashed border-border/70 p-4 text-sm text-muted-foreground">
              <ListChecks className="h-5 w-5 shrink-0" />
              Draft creation and edit routes are available now.
            </div>
          </CardContent>
        </Card>
      </div>
    </section>
  );
}
