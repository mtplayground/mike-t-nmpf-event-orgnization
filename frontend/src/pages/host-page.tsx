import { useEffect, useMemo, useState } from 'react';
import {
  CalendarPlus,
  Copy,
  Edit3,
  LoaderCircle,
  RefreshCw,
  Trash2,
  Users,
} from 'lucide-react';
import { Link } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { requestJson } from '@/lib/api-client';
import { cn } from '@/lib/utils';
import { useAuthStore } from '@/stores/auth-store';

type HostEventTab = 'draft' | 'upcoming' | 'past';
type EventLocationType = 'in_person' | 'virtual' | 'hybrid';
type EventVisibility = 'draft' | 'public' | 'unlisted' | 'private';
type EventStatus = 'draft' | 'published' | 'cancelled' | 'completed';

type HostEventListItem = {
  id: string;
  title: string;
  slug: string;
  description_md: string;
  start_at: string;
  end_at: string;
  timezone: string;
  location_type: EventLocationType;
  location_text: string | null;
  location_url: string | null;
  capacity: number | null;
  visibility: EventVisibility;
  status: EventStatus;
  attendee_count: number;
  cover_image_id: string | null;
  cancelled_at: string | null;
};

type HostEventListResponse = {
  items: HostEventListItem[];
  page: number;
  per_page: number;
  total_count: number;
  total_pages: number;
};

const tabs: Array<{ id: HostEventTab; label: string; description: string }> = [
  {
    id: 'upcoming',
    label: 'Upcoming',
    description: 'Published events that have not ended.',
  },
  {
    id: 'draft',
    label: 'Drafts',
    description: 'Unpublished events still being prepared.',
  },
  {
    id: 'past',
    label: 'Past',
    description: 'Completed events and published events that have ended.',
  },
];

const perPage = 10;

export function HostPage() {
  const refreshSession = useAuthStore((state) => state.refreshSession);
  const [activeTab, setActiveTab] = useState<HostEventTab>('upcoming');
  const [page, setPage] = useState(1);
  const [eventList, setEventList] = useState<HostEventListResponse | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [actionEventId, setActionEventId] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const activeTabDetails = useMemo(
    () => tabs.find((tab) => tab.id === activeTab) ?? tabs[0],
    [activeTab],
  );

  useEffect(() => {
    let cancelled = false;

    async function loadEvents() {
      setLoading(true);
      setError(null);

      try {
        const data = await authorizedRequest<HostEventListResponse>(
          `/me/events?status=${activeTab}&page=${page}&per_page=${perPage}`,
          refreshSession,
        );

        if (!cancelled) {
          setEventList(data);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(readError(loadError, 'Unable to load host events.'));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadEvents();

    return () => {
      cancelled = true;
    };
  }, [activeTab, page, refreshSession]);

  function selectTab(tab: HostEventTab) {
    setActiveTab(tab);
    setPage(1);
    setMessage(null);
    setError(null);
  }

  async function refreshEvents(clearMessage = true) {
    setLoading(true);
    if (clearMessage) {
      setMessage(null);
    }
    setError(null);

    try {
      const data = await authorizedRequest<HostEventListResponse>(
        `/me/events?status=${activeTab}&page=${page}&per_page=${perPage}`,
        refreshSession,
      );
      setEventList(data);
    } catch (refreshError) {
      setError(readError(refreshError, 'Unable to refresh host events.'));
    } finally {
      setLoading(false);
    }
  }

  async function cancelEvent(event: HostEventListItem) {
    const confirmed = window.confirm(`Cancel "${event.title}"?`);

    if (!confirmed) {
      return;
    }

    await runEventAction(event.id, async () => {
      await authorizedRequest(`/events/${event.id}`, refreshSession, {
        method: 'DELETE',
      });
      setMessage(`Cancelled "${event.title}".`);
      await refreshEvents(false);
    });
  }

  async function duplicateEvent(event: HostEventListItem) {
    await runEventAction(event.id, async () => {
      const duplicate = await authorizedRequest<HostEventListItem>(
        `/events/${event.id}/duplicate`,
        refreshSession,
        { method: 'POST' },
      );
      setMessage(`Duplicated "${event.title}" as "${duplicate.title}".`);
      if (activeTab === 'draft' && page === 1) {
        await refreshEvents(false);
      } else {
        setActiveTab('draft');
        setPage(1);
      }
    });
  }

  async function runEventAction(eventId: string, action: () => Promise<void>) {
    setActionEventId(eventId);
    setError(null);
    setMessage(null);

    try {
      await action();
    } catch (actionError) {
      setError(readError(actionError, 'Unable to update this event.'));
    } finally {
      setActionEventId(null);
    }
  }

  const totalCount = eventList?.total_count ?? 0;
  const totalPages = eventList?.total_pages ?? 0;
  const canGoBack = page > 1 && !loading;
  const canGoForward = totalPages > 0 && page < totalPages && !loading;

  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Host
            </p>
            <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
              Event dashboard
            </h2>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
              Review organizer events by status, watch attendance counts, and
              jump directly into event operations.
            </p>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button asChild>
              <Link to="/host/events/new">
                <CalendarPlus className="h-4 w-4" />
                New event
              </Link>
            </Button>
            <Button
              disabled={loading}
              onClick={() => refreshEvents()}
              variant="outline"
            >
              {loading ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              Refresh
            </Button>
          </div>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <CardTitle>{activeTabDetails.label} events</CardTitle>
              <CardDescription>{activeTabDetails.description}</CardDescription>
            </div>
            <div className="flex rounded-xl border border-border/70 bg-background/70 p-1">
              {tabs.map((tab) => (
                <button
                  className={cn(
                    'h-9 rounded-lg px-3 text-sm font-medium transition',
                    activeTab === tab.id
                      ? 'bg-primary text-primary-foreground'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                  )}
                  key={tab.id}
                  onClick={() => selectTab(tab.id)}
                  type="button"
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-3 text-sm text-muted-foreground">
            <span>
              {loading
                ? 'Loading events...'
                : `${totalCount} event${totalCount === 1 ? '' : 's'}`}
            </span>
            <span>
              Page {totalPages === 0 ? 0 : page} of {totalPages}
            </span>
          </div>

          {loading ? (
            <StatusPanel body="Fetching host event records." title="Loading" />
          ) : null}

          {!loading && eventList?.items.length === 0 ? (
            <EmptyState activeTab={activeTab} />
          ) : null}

          {!loading && eventList?.items.length ? (
            <div className="space-y-3">
              {eventList.items.map((event) => (
                <EventDashboardRow
                  actionEventId={actionEventId}
                  event={event}
                  key={event.id}
                  onCancel={cancelEvent}
                  onDuplicate={duplicateEvent}
                />
              ))}
            </div>
          ) : null}

          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border/70 pt-4">
            <div className="text-sm text-muted-foreground">
              Showing up to {perPage} events per page.
            </div>
            <div className="flex gap-2">
              <Button
                disabled={!canGoBack}
                onClick={() => setPage((current) => Math.max(1, current - 1))}
                type="button"
                variant="outline"
              >
                Previous
              </Button>
              <Button
                disabled={!canGoForward}
                onClick={() => setPage((current) => current + 1)}
                type="button"
                variant="outline"
              >
                Next
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {message ? (
        <StatusPanel body={message} title="Event updated" tone="success" />
      ) : null}
      {error ? (
        <StatusPanel
          body={error}
          title="Something needs attention"
          tone="error"
        />
      ) : null}
    </section>
  );
}

type EventDashboardRowProps = {
  event: HostEventListItem;
  actionEventId: string | null;
  onCancel: (event: HostEventListItem) => Promise<void>;
  onDuplicate: (event: HostEventListItem) => Promise<void>;
};

function EventDashboardRow({
  actionEventId,
  event,
  onCancel,
  onDuplicate,
}: EventDashboardRowProps) {
  const isActing = actionEventId === event.id;
  const canCancel =
    event.status !== 'cancelled' && event.status !== 'completed';

  return (
    <article className="rounded-2xl border border-border/70 bg-background/80 p-4 shadow-sm">
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_auto]">
        <div className="min-w-0 space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate text-lg font-semibold text-foreground">
              {event.title}
            </h3>
            <StatusBadge status={event.status} visibility={event.visibility} />
          </div>
          <div className="grid gap-2 text-sm text-muted-foreground md:grid-cols-2 xl:grid-cols-4">
            <Metric label="Starts" value={formatDateTime(event.start_at)} />
            <Metric label="Ends" value={formatDateTime(event.end_at)} />
            <Metric
              label="Attendance"
              value={`${event.attendee_count} attendee${event.attendee_count === 1 ? '' : 's'}`}
            />
            <Metric
              label="Capacity"
              value={event.capacity ? String(event.capacity) : 'Open'}
            />
          </div>
          <p className="line-clamp-2 text-sm leading-6 text-muted-foreground">
            {event.description_md.trim() || locationSummary(event)}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2 xl:justify-end">
          <Button asChild size="sm" variant="outline">
            <Link to={`/host/events/${event.id}/attendees`}>
              <Users className="h-4 w-4" />
              Attendees
            </Link>
          </Button>
          <Button asChild size="sm" variant="outline">
            <Link to={`/host/events/${event.id}/edit`}>
              <Edit3 className="h-4 w-4" />
              Edit
            </Link>
          </Button>
          <Button
            disabled={isActing}
            onClick={() => onDuplicate(event)}
            size="sm"
            type="button"
            variant="outline"
          >
            {isActing ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : (
              <Copy className="h-4 w-4" />
            )}
            Duplicate
          </Button>
          <Button
            disabled={!canCancel || isActing}
            onClick={() => onCancel(event)}
            size="sm"
            type="button"
            variant="outline"
          >
            {isActing ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : (
              <Trash2 className="h-4 w-4" />
            )}
            Cancel
          </Button>
        </div>
      </div>
    </article>
  );
}

function EmptyState({ activeTab }: { activeTab: HostEventTab }) {
  const message =
    activeTab === 'draft'
      ? 'No draft events yet.'
      : activeTab === 'upcoming'
        ? 'No upcoming events are published.'
        : 'No past events yet.';

  return (
    <div className="rounded-2xl border border-dashed border-border/70 p-5 text-sm text-muted-foreground">
      {message}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-secondary/35 px-3 py-2">
      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </p>
      <p className="mt-1 truncate font-medium text-foreground">{value}</p>
    </div>
  );
}

function StatusBadge({
  status,
  visibility,
}: {
  status: EventStatus;
  visibility: EventVisibility;
}) {
  return (
    <span className="rounded-full border border-border/70 bg-secondary/40 px-2.5 py-1 text-xs font-medium text-muted-foreground">
      {status.replace('_', ' ')} / {visibility}
    </span>
  );
}

type StatusPanelProps = {
  title: string;
  body: string;
  tone?: 'default' | 'success' | 'error';
};

function StatusPanel({ body, title, tone = 'default' }: StatusPanelProps) {
  return (
    <div
      className={cn(
        'rounded-2xl border p-4 text-sm leading-6',
        tone === 'success' &&
          'border-emerald-300/60 bg-emerald-500/10 text-emerald-900 dark:text-emerald-100',
        tone === 'error' &&
          'border-rose-300/60 bg-rose-500/10 text-rose-900 dark:text-rose-100',
        tone === 'default' &&
          'border-border/70 bg-background/80 text-muted-foreground',
      )}
    >
      <p className="font-medium">{title}</p>
      <p className="mt-1">{body}</p>
    </div>
  );
}

async function authorizedRequest<T>(
  path: string,
  refreshSession: () => Promise<boolean>,
  options: { method?: string; body?: unknown } = {},
) {
  const refreshed = await refreshSession();
  const session = useAuthStore.getState().session;

  if (!refreshed || !session) {
    throw new Error('You must be signed in to continue.');
  }

  return requestJson<T>(path, {
    ...options,
    token: session.accessToken,
  });
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function locationSummary(event: HostEventListItem) {
  if (event.location_type === 'virtual') {
    return event.location_url ?? 'Virtual event';
  }

  if (event.location_type === 'hybrid') {
    return [event.location_text, event.location_url]
      .filter(Boolean)
      .join(' + ');
  }

  return event.location_text ?? 'In-person event';
}

function readError(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallback;
}
