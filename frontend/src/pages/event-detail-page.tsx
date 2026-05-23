import { useEffect, useMemo, useState } from 'react';
import {
  CalendarDays,
  CheckCircle2,
  Clock,
  ImageIcon,
  MapPin,
  Share2,
  UserRound,
  Users,
  XCircle,
} from 'lucide-react';
import { Link, useParams } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { requestJson } from '@/lib/api-client';
import { cn } from '@/lib/utils';
import { useAuthStore } from '@/stores/auth-store';

type EventLocationType = 'in_person' | 'virtual' | 'hybrid';
type EventVisibility = 'draft' | 'public' | 'unlisted' | 'private';
type EventStatus = 'draft' | 'published' | 'cancelled' | 'completed';
type RegistrationState = 'registered' | 'cancelled' | 'waitlisted' | null;

type PublicEventThumbnail = {
  object_key: string;
  public_url: string | null;
  width: number;
  height: number;
  bytes: number;
};

type PublicEventDetailEvent = {
  id: string;
  host_id: string;
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
  thumbnail: PublicEventThumbnail | null;
  created_at: string;
  updated_at: string;
  cancelled_at: string | null;
};

type PublicEventHost = {
  id: string;
  display_name: string;
  avatar_object_key: string | null;
};

type PublicEventDetailResponse = {
  event: PublicEventDetailEvent;
  host: PublicEventHost;
  attendee_count: number;
  capacity_remaining: number | null;
  current_user_registration_state: RegistrationState;
};

type RegistrationResponse = {
  id: string;
  event_id: string;
  user_id: string;
  status: Exclude<RegistrationState, 'waitlisted' | null>;
  registered_at: string;
  cancelled_at: string | null;
};

type LocalRegistrationState = RegistrationState | 'pending';

export function EventDetailPage() {
  const { slug } = useParams();
  const refreshSession = useAuthStore((state) => state.refreshSession);
  const session = useAuthStore((state) => state.session);
  const [detail, setDetail] = useState<PublicEventDetailResponse | null>(null);
  const [registrationState, setRegistrationState] =
    useState<LocalRegistrationState>(null);
  const [loading, setLoading] = useState(true);
  const [registrationBusy, setRegistrationBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadEvent() {
      if (!slug) {
        setError('Missing event slug.');
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);

      try {
        const refreshed = await refreshSession();
        const currentSession = useAuthStore.getState().session;
        const data = await requestJson<PublicEventDetailResponse>(
          `/events/${encodeURIComponent(slug)}`,
          {
            token:
              refreshed && currentSession ? currentSession.accessToken : null,
          },
        );

        if (cancelled) {
          return;
        }

        setDetail(data);
        setRegistrationState(data.current_user_registration_state);
      } catch (loadError) {
        if (!cancelled) {
          setError(readError(loadError, 'Unable to load this event.'));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadEvent();

    return () => {
      cancelled = true;
    };
  }, [refreshSession, slug]);

  const shareUrl = useMemo(() => {
    if (!detail) {
      return '';
    }

    return `${window.location.origin}/events/${detail.event.slug}`;
  }, [detail]);

  async function shareEvent() {
    if (!detail || !shareUrl) {
      return;
    }

    setError(null);

    try {
      if (navigator.share) {
        await navigator.share({
          title: detail.event.title,
          text: detail.event.description_md || detail.event.title,
          url: shareUrl,
        });
      } else {
        await navigator.clipboard.writeText(shareUrl);
        setMessage('Event link copied.');
      }
    } catch (shareError) {
      if (
        shareError instanceof DOMException &&
        shareError.name === 'AbortError'
      ) {
        return;
      }

      setError(readError(shareError, 'Unable to share this event.'));
    }
  }

  async function register() {
    if (!detail) {
      return;
    }

    setError(null);
    setMessage(null);

    const previousDetail = detail;
    const previousRegistrationState = registrationState;
    const optimisticDetail = applyRegistrationDelta(detail, 1);

    setRegistrationBusy(true);
    setRegistrationState('pending');
    setDetail(optimisticDetail);

    try {
      const refreshed = await refreshSession({ force: true });
      const currentSession = useAuthStore.getState().session;

      if (!refreshed || !currentSession) {
        throw new Error('You must be signed in to register.');
      }

      const registration = await requestJson<RegistrationResponse>(
        `/events/${detail.event.id}/register`,
        {
          method: 'POST',
          token: currentSession.accessToken,
        },
      );

      setRegistrationState(registration.status);
      setMessage('Registration confirmed.');
    } catch (registerError) {
      setDetail(previousDetail);
      setRegistrationState(previousRegistrationState);
      setError(readError(registerError, 'Unable to register for this event.'));
    } finally {
      setRegistrationBusy(false);
    }
  }

  async function cancelRegistration() {
    if (!detail) {
      return;
    }

    setError(null);
    setMessage(null);

    const previousDetail = detail;
    const previousRegistrationState = registrationState;
    const optimisticDetail = applyRegistrationDelta(detail, -1);

    setRegistrationBusy(true);
    setRegistrationState(null);
    setDetail(optimisticDetail);

    try {
      const refreshed = await refreshSession({ force: true });
      const currentSession = useAuthStore.getState().session;

      if (!refreshed || !currentSession) {
        throw new Error('You must be signed in to cancel registration.');
      }

      await requestJson<RegistrationResponse>(
        `/events/${detail.event.id}/register`,
        {
          method: 'DELETE',
          token: currentSession.accessToken,
        },
      );

      setMessage('Registration cancelled.');
    } catch (cancelError) {
      setDetail(previousDetail);
      setRegistrationState(previousRegistrationState);
      setError(readError(cancelError, 'Unable to cancel this registration.'));
    } finally {
      setRegistrationBusy(false);
    }
  }

  if (loading) {
    return <DetailLoadingState />;
  }

  if (error && !detail) {
    return (
      <section className="space-y-4">
        <Button asChild variant="outline">
          <Link to="/attendee">Back to discovery</Link>
        </Button>
        <StatusPanel body={error} title="Event unavailable" tone="error" />
      </section>
    );
  }

  if (!detail) {
    return null;
  }

  return (
    <section className="space-y-6">
      <EventHero detail={detail} onShare={() => void shareEvent()} />

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <main className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Description</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="whitespace-pre-wrap text-sm leading-7 text-muted-foreground">
                {detail.event.description_md.trim() ||
                  'No description has been provided yet.'}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Event details</CardTitle>
            </CardHeader>
            <CardContent className="grid gap-3 md:grid-cols-2">
              <DetailMetric
                icon={CalendarDays}
                label="Starts"
                value={formatDateTime(detail.event.start_at)}
              />
              <DetailMetric
                icon={Clock}
                label="Ends"
                value={formatDateTime(detail.event.end_at)}
              />
              <DetailMetric
                icon={MapPin}
                label="Location"
                value={locationSummary(detail.event)}
              />
              <DetailMetric
                icon={Users}
                label="Attendance"
                value={`${detail.attendee_count} attending`}
              />
            </CardContent>
          </Card>
        </main>

        <aside className="space-y-6">
          <RegistrationPanel
            detail={detail}
            busy={registrationBusy}
            registrationState={registrationState}
            signedIn={Boolean(session)}
            onCancel={cancelRegistration}
            onRegister={register}
          />
          <HostCard host={detail.host} />
          {message ? (
            <StatusPanel body={message} title="Event action" tone="success" />
          ) : null}
          {error ? (
            <StatusPanel
              body={error}
              title="Something needs attention"
              tone="error"
            />
          ) : null}
        </aside>
      </div>
    </section>
  );
}

function EventHero({
  detail,
  onShare,
}: {
  detail: PublicEventDetailResponse;
  onShare: () => void;
}) {
  const imageUrl = detail.event.thumbnail?.public_url;

  return (
    <div className="overflow-hidden rounded-[28px] border border-border/60 bg-card/85 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
      <div className="grid lg:grid-cols-[minmax(0,1.35fr)_minmax(22rem,0.85fr)]">
        <div className="relative min-h-[22rem] bg-muted">
          {imageUrl ? (
            <img
              alt=""
              className="absolute inset-0 h-full w-full object-cover"
              height={detail.event.thumbnail?.height || 540}
              src={imageUrl}
              width={detail.event.thumbnail?.width || 960}
            />
          ) : (
            <div className="absolute inset-0 flex items-center justify-center bg-[linear-gradient(135deg,rgba(255,126,54,0.2),rgba(18,165,148,0.24))]">
              <ImageIcon className="h-14 w-14 text-primary" />
            </div>
          )}
        </div>
        <div className="flex min-h-[22rem] flex-col justify-between gap-6 p-6">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Public event
            </p>
            <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground md:text-4xl">
              {detail.event.title}
            </h2>
            <div className="mt-4 space-y-2 text-sm leading-6 text-muted-foreground">
              <p>{formatDateTime(detail.event.start_at)}</p>
              <p>{locationSummary(detail.event)}</p>
            </div>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button onClick={onShare} type="button">
              <Share2 className="h-4 w-4" />
              Share
            </Button>
            <Button asChild variant="outline">
              <Link to="/attendee">Back to discovery</Link>
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function RegistrationPanel({
  busy,
  detail,
  registrationState,
  signedIn,
  onCancel,
  onRegister,
}: {
  busy: boolean;
  detail: PublicEventDetailResponse;
  registrationState: LocalRegistrationState;
  signedIn: boolean;
  onCancel: () => void;
  onRegister: () => void;
}) {
  const capacityPercent = capacityUsagePercent(detail);
  const isRegistered =
    registrationState === 'registered' ||
    registrationState === 'waitlisted' ||
    registrationState === 'pending';
  const isFull =
    detail.capacity_remaining !== null &&
    detail.capacity_remaining <= 0 &&
    !isRegistered;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Registration</CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-3 text-sm">
            <span className="text-muted-foreground">Capacity</span>
            <span className="font-medium text-foreground">
              {capacityLabel(detail)}
            </span>
          </div>
          <div className="h-3 overflow-hidden rounded-full bg-secondary">
            <div
              className="h-full rounded-full bg-primary transition-all"
              style={{ width: `${capacityPercent}%` }}
            />
          </div>
          <p className="text-xs text-muted-foreground">
            {remainingCapacityLabel(detail)}
          </p>
        </div>

        <div className="rounded-xl border border-border/70 bg-background/70 p-3 text-sm">
          <div className="flex items-center gap-2 font-medium">
            {isRegistered ? (
              <CheckCircle2 className="h-4 w-4 text-emerald-600" />
            ) : (
              <XCircle className="h-4 w-4 text-muted-foreground" />
            )}
            {registrationLabel(registrationState)}
          </div>
        </div>

        {signedIn ? (
          isRegistered ? (
            <Button
              className="w-full"
              disabled={busy}
              onClick={onCancel}
              type="button"
              variant="outline"
            >
              {busy ? 'Cancelling...' : 'Cancel registration'}
            </Button>
          ) : (
            <Button
              className="w-full"
              disabled={isFull || busy}
              onClick={onRegister}
              type="button"
            >
              {busy ? 'Registering...' : isFull ? 'Event is full' : 'Register'}
            </Button>
          )
        ) : (
          <Button asChild className="w-full">
            <Link to="/auth/login">Sign in to register</Link>
          </Button>
        )}
        {isFull ? (
          <p className="text-xs leading-5 text-muted-foreground">
            Capacity has been reached. Try again later if another attendee
            cancels.
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}

function HostCard({ host }: { host: PublicEventHost }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Host</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-secondary text-secondary-foreground">
            <UserRound className="h-5 w-5" />
          </div>
          <div className="min-w-0">
            <p className="truncate font-medium text-foreground">
              {host.display_name}
            </p>
            <p className="truncate text-sm text-muted-foreground">{host.id}</p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function DetailMetric({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof CalendarDays;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-xl border border-border/70 bg-background/70 p-4">
      <div className="flex items-center gap-2 text-xs uppercase tracking-[0.18em] text-muted-foreground">
        <Icon className="h-4 w-4 text-primary" />
        {label}
      </div>
      <p className="mt-2 text-sm font-medium leading-6 text-foreground">
        {value}
      </p>
    </div>
  );
}

function DetailLoadingState() {
  return (
    <section className="space-y-6">
      <div className="overflow-hidden rounded-[28px] border border-border/60 bg-card/85">
        <div className="grid lg:grid-cols-[minmax(0,1.35fr)_minmax(22rem,0.85fr)]">
          <div className="min-h-[22rem] animate-pulse bg-muted" />
          <div className="space-y-4 p-6">
            <div className="h-4 w-28 animate-pulse rounded bg-muted" />
            <div className="h-10 w-3/4 animate-pulse rounded bg-muted" />
            <div className="h-4 w-48 animate-pulse rounded bg-muted" />
            <div className="h-10 w-32 animate-pulse rounded bg-muted" />
          </div>
        </div>
      </div>
    </section>
  );
}

function StatusPanel({
  body,
  title,
  tone,
}: {
  title: string;
  body: string;
  tone: 'success' | 'error';
}) {
  return (
    <div
      className={cn(
        'rounded-2xl border p-4 text-sm leading-6',
        tone === 'success' &&
          'border-emerald-300/60 bg-emerald-500/10 text-emerald-900 dark:text-emerald-100',
        tone === 'error' &&
          'border-rose-300/60 bg-rose-500/10 text-rose-900 dark:text-rose-100',
      )}
    >
      <p className="font-medium">{title}</p>
      <p className="mt-1">{body}</p>
    </div>
  );
}

function capacityUsagePercent(detail: PublicEventDetailResponse) {
  if (!detail.event.capacity) {
    return 0;
  }

  return Math.min(
    100,
    Math.round((detail.attendee_count / detail.event.capacity) * 100),
  );
}

function capacityLabel(detail: PublicEventDetailResponse) {
  return detail.event.capacity
    ? `${detail.attendee_count} / ${detail.event.capacity}`
    : `${detail.attendee_count} attending`;
}

function remainingCapacityLabel(detail: PublicEventDetailResponse) {
  if (detail.capacity_remaining === null) {
    return 'Open capacity';
  }

  return `${detail.capacity_remaining} spot${detail.capacity_remaining === 1 ? '' : 's'} remaining`;
}

function applyRegistrationDelta(
  detail: PublicEventDetailResponse,
  delta: 1 | -1,
): PublicEventDetailResponse {
  const attendeeCount = Math.max(0, detail.attendee_count + delta);
  const capacityRemaining =
    detail.capacity_remaining === null
      ? null
      : Math.max(0, detail.capacity_remaining - delta);

  return {
    ...detail,
    attendee_count: attendeeCount,
    capacity_remaining: capacityRemaining,
  };
}

function registrationLabel(registrationState: LocalRegistrationState) {
  if (registrationState === 'registered') {
    return 'You are registered';
  }

  if (registrationState === 'waitlisted') {
    return 'You are waitlisted';
  }

  if (registrationState === 'pending') {
    return 'Registration selected';
  }

  return 'Not registered';
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function locationSummary(event: PublicEventDetailEvent) {
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
