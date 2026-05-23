import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from 'react';
import {
  CalendarDays,
  ImageIcon,
  LoaderCircle,
  MapPin,
  RefreshCw,
  Search,
} from 'lucide-react';
import { Link } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { requestJson } from '@/lib/api-client';

type EventLocationType = 'in_person' | 'virtual' | 'hybrid';

type PublicEventThumbnail = {
  object_key: string;
  public_url: string | null;
  width: number;
  height: number;
  bytes: number;
};

type PublicEvent = {
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
  thumbnail: PublicEventThumbnail | null;
};

type PublicEventListResponse = {
  items: PublicEvent[];
  next_cursor: string | null;
};

type DiscoveryFilters = {
  query: string;
  fromDate: string;
};

export function AttendeePage() {
  const [queryInput, setQueryInput] = useState('');
  const [dateInput, setDateInput] = useState('');
  const [filters, setFilters] = useState<DiscoveryFilters>({
    query: '',
    fromDate: '',
  });
  const [events, setEvents] = useState<PublicEvent[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  const fetchEvents = useCallback(
    async (options: { cursor?: string | null; replace: boolean }) => {
      const { cursor = null, replace } = options;

      if (replace) {
        setLoading(true);
      } else {
        setLoadingMore(true);
      }
      setError(null);

      try {
        const params = publicEventSearchParams(filters, cursor);
        const suffix = params.toString();
        const data = await requestJson<PublicEventListResponse>(
          suffix ? `/events?${suffix}` : '/events',
        );

        setEvents((current) =>
          replace ? data.items : mergeEvents(current, data.items),
        );
        setNextCursor(data.next_cursor);
      } catch (loadError) {
        setError(readError(loadError, 'Unable to load upcoming events.'));
      } finally {
        if (replace) {
          setLoading(false);
        } else {
          setLoadingMore(false);
        }
      }
    },
    [filters],
  );

  useEffect(() => {
    void fetchEvents({ replace: true });
  }, [fetchEvents]);

  useEffect(() => {
    const sentinel = sentinelRef.current;

    if (!sentinel || !nextCursor || loading || loadingMore) {
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          void fetchEvents({ cursor: nextCursor, replace: false });
        }
      },
      { rootMargin: '320px 0px' },
    );

    observer.observe(sentinel);

    return () => observer.disconnect();
  }, [fetchEvents, loading, loadingMore, nextCursor]);

  function applyFilters(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setFilters({ query: queryInput.trim(), fromDate: dateInput });
  }

  function clearFilters() {
    setQueryInput('');
    setDateInput('');
    setFilters({ query: '', fromDate: '' });
  }

  const hasFilters = filters.query !== '' || filters.fromDate !== '';

  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Attendee
            </p>
            <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
              Discover upcoming events
            </h2>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
              Browse public events by date, venue, and topic.
            </p>
          </div>
          <Button
            disabled={loading}
            onClick={() => void fetchEvents({ replace: true })}
            type="button"
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

      <Card>
        <CardContent className="space-y-5 pt-6">
          <form
            className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_14rem_auto_auto]"
            onSubmit={applyFilters}
          >
            <label className="relative block">
              <Search className="pointer-events-none absolute left-3 top-3.5 h-4 w-4 text-muted-foreground" />
              <Input
                aria-label="Search events"
                className="pl-9"
                onChange={(event) => setQueryInput(event.target.value)}
                placeholder="Search events"
                value={queryInput}
              />
            </label>
            <label className="relative block">
              <CalendarDays className="pointer-events-none absolute left-3 top-3.5 h-4 w-4 text-muted-foreground" />
              <Input
                aria-label="Filter by earliest date"
                className="pl-9"
                onChange={(event) => setDateInput(event.target.value)}
                type="date"
                value={dateInput}
              />
            </label>
            <Button type="submit">
              <Search className="h-4 w-4" />
              Search
            </Button>
            <Button
              disabled={!queryInput && !dateInput && !hasFilters}
              onClick={clearFilters}
              type="button"
              variant="outline"
            >
              Clear
            </Button>
          </form>

          <div className="flex flex-wrap items-center justify-between gap-3 text-sm text-muted-foreground">
            <span>
              {loading
                ? 'Loading events...'
                : `${events.length} event${events.length === 1 ? '' : 's'} loaded`}
            </span>
            {hasFilters ? <span>{activeFilterSummary(filters)}</span> : null}
          </div>
        </CardContent>
      </Card>

      {error ? (
        <StatusPanel body={error} title="Something needs attention" />
      ) : null}

      {loading ? <EventGridSkeleton /> : null}

      {!loading && events.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 bg-background/70 p-6 text-sm text-muted-foreground">
          No upcoming public events match the current filters.
        </div>
      ) : null}

      {!loading && events.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {events.map((event) => (
            <DiscoveryEventCard event={event} key={event.id} />
          ))}
        </div>
      ) : null}

      <div ref={sentinelRef} />

      {loadingMore ? (
        <div className="flex items-center justify-center gap-2 py-4 text-sm text-muted-foreground">
          <LoaderCircle className="h-4 w-4 animate-spin" />
          Loading more events...
        </div>
      ) : null}

      {!loading && !loadingMore && events.length > 0 && !nextCursor ? (
        <div className="py-4 text-center text-sm text-muted-foreground">
          End of results.
        </div>
      ) : null}
    </section>
  );
}

function DiscoveryEventCard({ event }: { event: PublicEvent }) {
  return (
    <article className="overflow-hidden rounded-2xl border border-border/70 bg-card/90 shadow-sm">
      <EventThumbnail event={event} />
      <div className="space-y-4 p-4">
        <div className="space-y-2">
          <p className="text-sm font-medium text-primary">
            {formatDateTime(event.start_at)}
          </p>
          <h3 className="line-clamp-2 text-xl font-semibold tracking-tight text-foreground">
            {event.title}
          </h3>
          <p className="line-clamp-3 text-sm leading-6 text-muted-foreground">
            {event.description_md.trim() || locationSummary(event)}
          </p>
        </div>

        <div className="space-y-2 text-sm text-muted-foreground">
          <div className="flex items-start gap-2">
            <MapPin className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            <span className="line-clamp-2">{locationSummary(event)}</span>
          </div>
          <div className="flex items-start gap-2">
            <CalendarDays className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
            <span>{durationSummary(event)}</span>
          </div>
        </div>

        <div className="flex items-center justify-between gap-3 border-t border-border/70 pt-4">
          <span className="rounded-full border border-border/70 bg-secondary/40 px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {event.capacity ? `${event.capacity} seats` : 'Open capacity'}
          </span>
          <Button asChild size="sm" variant="outline">
            <Link to={`/events/${event.slug}`}>Details</Link>
          </Button>
        </div>
      </div>
    </article>
  );
}

function EventThumbnail({ event }: { event: PublicEvent }) {
  const publicUrl = event.thumbnail?.public_url;

  if (publicUrl) {
    return (
      <img
        alt=""
        className="aspect-[16/9] w-full object-cover"
        height={event.thumbnail?.height || 270}
        src={publicUrl}
        width={event.thumbnail?.width || 480}
      />
    );
  }

  return (
    <div className="flex aspect-[16/9] items-center justify-center bg-[linear-gradient(135deg,rgba(255,126,54,0.18),rgba(18,165,148,0.24))]">
      <div className="flex h-16 w-16 items-center justify-center rounded-full border border-border/70 bg-background/75">
        <ImageIcon className="h-7 w-7 text-primary" />
      </div>
    </div>
  );
}

function EventGridSkeleton() {
  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
      {Array.from({ length: 6 }).map((_, index) => (
        <div
          className="overflow-hidden rounded-2xl border border-border/70 bg-card/80"
          key={index}
        >
          <div className="aspect-[16/9] animate-pulse bg-muted" />
          <div className="space-y-3 p-4">
            <div className="h-4 w-28 animate-pulse rounded bg-muted" />
            <div className="h-6 w-3/4 animate-pulse rounded bg-muted" />
            <div className="h-4 w-full animate-pulse rounded bg-muted" />
            <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
          </div>
        </div>
      ))}
    </div>
  );
}

function StatusPanel({ body, title }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-rose-300/60 bg-rose-500/10 p-4 text-sm leading-6 text-rose-900 dark:text-rose-100">
      <p className="font-medium">{title}</p>
      <p className="mt-1">{body}</p>
    </div>
  );
}

function publicEventSearchParams(
  filters: DiscoveryFilters,
  cursor: string | null,
) {
  const params = new URLSearchParams();

  if (filters.query) {
    params.set('query', filters.query);
  }

  if (filters.fromDate) {
    params.set('from', new Date(`${filters.fromDate}T00:00:00`).toISOString());
  }

  if (cursor) {
    params.set('cursor', cursor);
  }

  return params;
}

function mergeEvents(current: PublicEvent[], next: PublicEvent[]) {
  const seen = new Set(current.map((event) => event.id));
  const merged = [...current];

  for (const event of next) {
    if (!seen.has(event.id)) {
      merged.push(event);
      seen.add(event.id);
    }
  }

  return merged;
}

function activeFilterSummary(filters: DiscoveryFilters) {
  const parts = [];

  if (filters.query) {
    parts.push(`"${filters.query}"`);
  }

  if (filters.fromDate) {
    parts.push(`from ${formatDate(filters.fromDate)}`);
  }

  return parts.join(' · ');
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(
    new Date(`${value}T00:00:00`),
  );
}

function durationSummary(event: PublicEvent) {
  return `${formatDateTime(event.start_at)} - ${formatDateTime(event.end_at)} (${event.timezone})`;
}

function locationSummary(event: PublicEvent) {
  if (event.location_type === 'virtual') {
    return event.location_url ?? 'Virtual event';
  }

  if (event.location_type === 'hybrid') {
    return event.location_text && event.location_url
      ? `${event.location_text} and virtual`
      : event.location_text || event.location_url || 'Hybrid event';
  }

  return event.location_text ?? 'Location to be announced';
}

function readError(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}
