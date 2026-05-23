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
  Inbox,
  LoaderCircle,
  MapPin,
  RefreshCw,
  Search,
  XCircle,
} from 'lucide-react';
import { Link } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { EmptyStatePanel } from '@/components/state-panels';
import { requestJson } from '@/lib/api-client';
import { useAuthStore } from '@/stores/auth-store';

type EventLocationType = 'in_person' | 'virtual' | 'hybrid';
type EventVisibility = 'draft' | 'public' | 'unlisted' | 'private';
type EventStatus = 'draft' | 'published' | 'cancelled' | 'completed';
type RegistrationStatus = 'registered' | 'cancelled';
type RegistrationBucket = 'upcoming' | 'past';

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

type AttendeeRegistrationEvent = {
  id: string;
  host_id: string;
  host_display_name: string;
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
  cover_image_id: string | null;
  cancelled_at: string | null;
};

type AttendeeRegistration = {
  registration_id: string;
  status: RegistrationStatus;
  registered_at: string;
  cancelled_at: string | null;
  event: AttendeeRegistrationEvent;
};

type PublicEventListResponse = {
  items: PublicEvent[];
  next_cursor: string | null;
};

type RegistrationListResponse = {
  items: AttendeeRegistration[];
  page: number;
  per_page: number;
  total_count: number;
  total_pages: number;
};

type DiscoveryFilters = {
  query: string;
  fromDate: string;
};

type EventSummaryLike = Pick<
  PublicEvent,
  | 'end_at'
  | 'location_text'
  | 'location_type'
  | 'location_url'
  | 'start_at'
  | 'timezone'
>;

export function AttendeePage() {
  const refreshSession = useAuthStore((state) => state.refreshSession);
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
  const [registrationBucket, setRegistrationBucket] =
    useState<RegistrationBucket>('upcoming');
  const [registrations, setRegistrations] = useState<AttendeeRegistration[]>(
    [],
  );
  const [registrationPage, setRegistrationPage] = useState(1);
  const [registrationTotalPages, setRegistrationTotalPages] = useState(0);
  const [registrationTotalCount, setRegistrationTotalCount] = useState(0);
  const [registrationsLoading, setRegistrationsLoading] = useState(true);
  const [registrationsLoadingMore, setRegistrationsLoadingMore] =
    useState(false);
  const [dashboardError, setDashboardError] = useState<string | null>(null);
  const [dashboardMessage, setDashboardMessage] = useState<string | null>(null);
  const [cancellingRegistrationId, setCancellingRegistrationId] = useState<
    string | null
  >(null);

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

  const fetchRegistrations = useCallback(
    async (options: {
      bucket: RegistrationBucket;
      page: number;
      replace: boolean;
    }) => {
      const { bucket, page, replace } = options;

      if (replace) {
        setRegistrationsLoading(true);
      } else {
        setRegistrationsLoadingMore(true);
      }
      setDashboardError(null);

      try {
        const refreshed = await refreshSession();
        const session = useAuthStore.getState().session;

        if (!refreshed || !session) {
          throw new Error('You must be signed in to view registrations.');
        }

        const params = new URLSearchParams({
          bucket,
          page: String(page),
          per_page: '6',
        });
        const data = await requestJson<RegistrationListResponse>(
          `/me/registrations?${params.toString()}`,
          { token: session.accessToken },
        );

        setRegistrations((current) =>
          replace ? data.items : mergeRegistrations(current, data.items),
        );
        setRegistrationPage(data.page);
        setRegistrationTotalPages(data.total_pages);
        setRegistrationTotalCount(data.total_count);
      } catch (loadError) {
        setDashboardError(
          readError(loadError, 'Unable to load registered events.'),
        );
      } finally {
        if (replace) {
          setRegistrationsLoading(false);
        } else {
          setRegistrationsLoadingMore(false);
        }
      }
    },
    [refreshSession],
  );

  useEffect(() => {
    void fetchEvents({ replace: true });
  }, [fetchEvents]);

  useEffect(() => {
    void fetchRegistrations({
      bucket: registrationBucket,
      page: 1,
      replace: true,
    });
  }, [fetchRegistrations, registrationBucket]);

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

  async function cancelRegistration(registration: AttendeeRegistration) {
    const previousRegistrations = registrations;
    const previousTotalCount = registrationTotalCount;

    setDashboardError(null);
    setDashboardMessage(null);
    setCancellingRegistrationId(registration.registration_id);
    setRegistrations((current) =>
      current.filter(
        (item) => item.registration_id !== registration.registration_id,
      ),
    );
    setRegistrationTotalCount((current) => Math.max(0, current - 1));

    try {
      const refreshed = await refreshSession({ force: true });
      const session = useAuthStore.getState().session;

      if (!refreshed || !session) {
        throw new Error('You must be signed in to cancel registration.');
      }

      await requestJson(`/events/${registration.event.id}/register`, {
        method: 'DELETE',
        token: session.accessToken,
      });

      setDashboardMessage(
        `Registration cancelled for ${registration.event.title}.`,
      );
    } catch (cancelError) {
      setRegistrations(previousRegistrations);
      setRegistrationTotalCount(previousTotalCount);
      setDashboardError(
        readError(cancelError, 'Unable to cancel this registration.'),
      );
    } finally {
      setCancellingRegistrationId(null);
    }
  }

  const hasFilters = filters.query !== '' || filters.fromDate !== '';
  const canLoadMoreRegistrations = registrationPage < registrationTotalPages;

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

      <RegistrationDashboard
        bucket={registrationBucket}
        canLoadMore={canLoadMoreRegistrations}
        cancellingRegistrationId={cancellingRegistrationId}
        error={dashboardError}
        loading={registrationsLoading}
        loadingMore={registrationsLoadingMore}
        message={dashboardMessage}
        onBucketChange={setRegistrationBucket}
        onCancel={(registration) => void cancelRegistration(registration)}
        onLoadMore={() =>
          void fetchRegistrations({
            bucket: registrationBucket,
            page: registrationPage + 1,
            replace: false,
          })
        }
        registrations={registrations}
        totalCount={registrationTotalCount}
      />

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
        <EmptyStatePanel
          body="Adjust the search or date filter to broaden discovery."
          icon={Inbox}
          title="No upcoming public events match the current filters."
        />
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

function RegistrationDashboard({
  bucket,
  canLoadMore,
  cancellingRegistrationId,
  error,
  loading,
  loadingMore,
  message,
  onBucketChange,
  onCancel,
  onLoadMore,
  registrations,
  totalCount,
}: {
  bucket: RegistrationBucket;
  canLoadMore: boolean;
  cancellingRegistrationId: string | null;
  error: string | null;
  loading: boolean;
  loadingMore: boolean;
  message: string | null;
  onBucketChange: (bucket: RegistrationBucket) => void;
  onCancel: (registration: AttendeeRegistration) => void;
  onLoadMore: () => void;
  registrations: AttendeeRegistration[];
  totalCount: number;
}) {
  return (
    <Card>
      <CardContent className="space-y-5 pt-6">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
              My registrations
            </p>
            <h3 className="mt-2 text-2xl font-semibold tracking-tight text-foreground">
              Registered events
            </h3>
          </div>
          <div className="flex rounded-full border border-border/70 bg-secondary/40 p-1">
            {(['upcoming', 'past'] as RegistrationBucket[]).map((item) => (
              <button
                className={[
                  'rounded-full px-4 py-2 text-sm font-medium transition',
                  bucket === item
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:text-foreground',
                ].join(' ')}
                key={item}
                onClick={() => onBucketChange(item)}
                type="button"
              >
                {item === 'upcoming' ? 'Upcoming' : 'Past'}
              </button>
            ))}
          </div>
        </div>

        {message ? <InlineStatus body={message} tone="success" /> : null}
        {error ? <InlineStatus body={error} tone="error" /> : null}

        <div className="text-sm text-muted-foreground">
          {loading
            ? 'Loading registrations...'
            : `${totalCount} ${bucket} registration${totalCount === 1 ? '' : 's'}`}
        </div>

        {loading ? <RegistrationListSkeleton /> : null}

        {!loading && registrations.length === 0 ? (
          <EmptyStatePanel
            body={
              bucket === 'upcoming'
                ? 'Register for an event to keep it visible here.'
                : 'Past registrations appear here after an event has ended.'
            }
            icon={Inbox}
            title={
              bucket === 'upcoming'
                ? 'No upcoming registered events yet.'
                : 'No past registered events yet.'
            }
          />
        ) : null}

        {!loading && registrations.length > 0 ? (
          <div className="grid gap-3">
            {registrations.map((registration) => (
              <RegistrationCard
                bucket={bucket}
                cancelling={
                  cancellingRegistrationId === registration.registration_id
                }
                key={registration.registration_id}
                onCancel={() => onCancel(registration)}
                registration={registration}
              />
            ))}
          </div>
        ) : null}

        {canLoadMore ? (
          <Button
            disabled={loadingMore}
            onClick={onLoadMore}
            type="button"
            variant="outline"
          >
            {loadingMore ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4" />
            )}
            Load more registrations
          </Button>
        ) : null}
      </CardContent>
    </Card>
  );
}

function RegistrationCard({
  bucket,
  cancelling,
  onCancel,
  registration,
}: {
  bucket: RegistrationBucket;
  cancelling: boolean;
  onCancel: () => void;
  registration: AttendeeRegistration;
}) {
  const event = registration.event;

  return (
    <article className="rounded-2xl border border-border/70 bg-background/70 p-4">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div className="min-w-0 space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded-full border border-border/70 bg-secondary/50 px-2.5 py-1 text-xs font-medium text-muted-foreground">
              {bucket === 'upcoming' ? 'Upcoming' : 'Past'}
            </span>
            <span className="rounded-full border border-border/70 bg-secondary/50 px-2.5 py-1 text-xs font-medium text-muted-foreground">
              {event.host_display_name}
            </span>
          </div>
          <div>
            <h4 className="truncate text-lg font-semibold text-foreground">
              {event.title}
            </h4>
            <p className="mt-1 text-sm text-muted-foreground">
              {durationSummary(event)}
            </p>
            <p className="mt-1 line-clamp-1 text-sm text-muted-foreground">
              {locationSummary(event)}
            </p>
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <Button asChild size="sm" variant="outline">
            <Link to={`/events/${event.slug}`}>Details</Link>
          </Button>
          {bucket === 'upcoming' ? (
            <Button
              disabled={cancelling}
              onClick={onCancel}
              size="sm"
              type="button"
              variant="outline"
            >
              {cancelling ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : (
                <XCircle className="h-4 w-4" />
              )}
              Cancel
            </Button>
          ) : null}
        </div>
      </div>
    </article>
  );
}

function RegistrationListSkeleton() {
  return (
    <div className="grid gap-3">
      {Array.from({ length: 3 }).map((_, index) => (
        <div
          className="rounded-2xl border border-border/70 bg-background/70 p-4"
          key={index}
        >
          <div className="h-4 w-32 animate-pulse rounded bg-muted" />
          <div className="mt-3 h-6 w-2/3 animate-pulse rounded bg-muted" />
          <div className="mt-3 h-4 w-1/2 animate-pulse rounded bg-muted" />
        </div>
      ))}
    </div>
  );
}

function InlineStatus({
  body,
  tone,
}: {
  body: string;
  tone: 'success' | 'error';
}) {
  return (
    <div
      className={[
        'rounded-2xl border p-4 text-sm leading-6',
        tone === 'success'
          ? 'border-emerald-300/60 bg-emerald-500/10 text-emerald-900 dark:text-emerald-100'
          : 'border-rose-300/60 bg-rose-500/10 text-rose-900 dark:text-rose-100',
      ].join(' ')}
    >
      {body}
    </div>
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

function mergeRegistrations(
  current: AttendeeRegistration[],
  next: AttendeeRegistration[],
) {
  const seen = new Set(
    current.map((registration) => registration.registration_id),
  );
  const merged = [...current];

  for (const registration of next) {
    if (!seen.has(registration.registration_id)) {
      merged.push(registration);
      seen.add(registration.registration_id);
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

function durationSummary(event: EventSummaryLike) {
  return `${formatDateTime(event.start_at)} - ${formatDateTime(event.end_at)} (${event.timezone})`;
}

function locationSummary(event: EventSummaryLike) {
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
