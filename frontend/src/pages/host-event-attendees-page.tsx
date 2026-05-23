import { useEffect, useMemo, useState, type FormEvent } from 'react';
import {
  Download,
  LoaderCircle,
  MailPlus,
  RefreshCw,
  Send,
  Users,
  X,
} from 'lucide-react';
import { Link, useParams } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { requestJson } from '@/lib/api-client';
import { appConfig } from '@/lib/config';
import { cn } from '@/lib/utils';
import { useAuthStore } from '@/stores/auth-store';

type RegistrationStatus = 'registered' | 'cancelled';

type EventResponse = {
  id: string;
  title: string;
  slug: string;
  capacity: number | null;
  start_at: string;
  end_at: string;
};

type HostEventAttendee = {
  registration_id: string;
  user_id: string;
  email: string;
  display_name: string;
  status: RegistrationStatus;
  registered_at: string;
  cancelled_at: string | null;
};

type HostEventAttendeesResponse = {
  event_id: string;
  attendees: HostEventAttendee[];
};

type AnnouncementResponse = {
  accepted: boolean;
  recipient_count: number;
};

export function HostEventAttendeesPage() {
  const { eventId } = useParams();
  const refreshSession = useAuthStore((state) => state.refreshSession);
  const [event, setEvent] = useState<EventResponse | null>(null);
  const [attendees, setAttendees] = useState<HostEventAttendee[]>([]);
  const [loading, setLoading] = useState(true);
  const [downloading, setDownloading] = useState(false);
  const [composerOpen, setComposerOpen] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const activeAttendeeCount = useMemo(
    () =>
      attendees.filter((attendee) => attendee.status === 'registered').length,
    [attendees],
  );

  useEffect(() => {
    void loadWorkspace();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [eventId]);

  async function loadWorkspace(clearMessage = true) {
    if (!eventId) {
      setError('Missing event id.');
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    if (clearMessage) {
      setMessage(null);
    }

    try {
      const [eventData, attendeeData] = await Promise.all([
        authorizedRequest<EventResponse>(
          `/me/events/${eventId}`,
          refreshSession,
        ),
        authorizedRequest<HostEventAttendeesResponse>(
          `/events/${eventId}/attendees`,
          refreshSession,
        ),
      ]);

      setEvent(eventData);
      setAttendees(attendeeData.attendees);
    } catch (loadError) {
      setError(readError(loadError, 'Unable to load attendees.'));
    } finally {
      setLoading(false);
    }
  }

  async function downloadCsv() {
    if (!eventId) {
      return;
    }

    setDownloading(true);
    setError(null);

    try {
      const blob = await authorizedBlobRequest(
        `/events/${eventId}/attendees.csv`,
        refreshSession,
      );
      const objectUrl = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = `${event?.slug ?? eventId}-attendees.csv`;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(objectUrl);
    } catch (downloadError) {
      setError(readError(downloadError, 'Unable to download attendee CSV.'));
    } finally {
      setDownloading(false);
    }
  }

  async function sendAnnouncement(subject: string, body: string) {
    if (!eventId) {
      return;
    }

    const response = await authorizedRequest<AnnouncementResponse>(
      `/events/${eventId}/announce`,
      refreshSession,
      {
        method: 'POST',
        body: { subject, body },
      },
    );

    setMessage(
      `Announcement queued for ${response.recipient_count} attendee${response.recipient_count === 1 ? '' : 's'}.`,
    );
    setComposerOpen(false);
  }

  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
              Host
            </p>
            <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
              {event?.title ?? 'Attendee management'}
            </h2>
            <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
              {activeAttendeeCount} active registration
              {activeAttendeeCount === 1 ? '' : 's'}
              {event?.capacity ? ` out of ${event.capacity} seats` : ''}
            </p>
          </div>
          <div className="flex flex-wrap gap-3">
            <Button
              disabled={loading}
              onClick={() => void loadWorkspace()}
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
            <Button
              disabled={downloading || loading}
              onClick={() => void downloadCsv()}
              type="button"
              variant="outline"
            >
              {downloading ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : (
                <Download className="h-4 w-4" />
              )}
              CSV
            </Button>
            <Button
              disabled={loading || activeAttendeeCount === 0}
              onClick={() => setComposerOpen(true)}
              type="button"
            >
              <MailPlus className="h-4 w-4" />
              Announce
            </Button>
            <Button asChild variant="outline">
              <Link to="/host">Back</Link>
            </Button>
          </div>
        </div>
      </div>

      {message ? (
        <StatusPanel body={message} title="Announcement" tone="success" />
      ) : null}
      {error ? (
        <StatusPanel
          body={error}
          title="Something needs attention"
          tone="error"
        />
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>Attendees</CardTitle>
          <CardDescription>
            Registered and cancelled attendees for this event.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {loading ? (
            <LoadingTable />
          ) : attendees.length === 0 ? (
            <div className="flex items-center gap-3 rounded-2xl border border-dashed border-border/70 p-5 text-sm text-muted-foreground">
              <Users className="h-5 w-5 shrink-0" />
              No registrations yet.
            </div>
          ) : (
            <AttendeesTable attendees={attendees} />
          )}
        </CardContent>
      </Card>

      {composerOpen ? (
        <AnnouncementModal
          eventTitle={event?.title ?? 'this event'}
          onClose={() => setComposerOpen(false)}
          onSend={sendAnnouncement}
        />
      ) : null}
    </section>
  );
}

function AttendeesTable({ attendees }: { attendees: HostEventAttendee[] }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-border/70">
      <div className="overflow-x-auto">
        <table className="w-full min-w-[46rem] text-left text-sm">
          <thead className="bg-secondary/50 text-xs uppercase tracking-[0.18em] text-muted-foreground">
            <tr>
              <th className="px-4 py-3 font-medium">Name</th>
              <th className="px-4 py-3 font-medium">Email</th>
              <th className="px-4 py-3 font-medium">Status</th>
              <th className="px-4 py-3 font-medium">Registered</th>
              <th className="px-4 py-3 font-medium">Cancelled</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border/70">
            {attendees.map((attendee) => (
              <tr key={attendee.registration_id}>
                <td className="px-4 py-3 font-medium text-foreground">
                  {attendee.display_name}
                </td>
                <td className="px-4 py-3 text-muted-foreground">
                  {attendee.email}
                </td>
                <td className="px-4 py-3">
                  <StatusBadge status={attendee.status} />
                </td>
                <td className="px-4 py-3 text-muted-foreground">
                  {formatDateTime(attendee.registered_at)}
                </td>
                <td className="px-4 py-3 text-muted-foreground">
                  {attendee.cancelled_at
                    ? formatDateTime(attendee.cancelled_at)
                    : '-'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function AnnouncementModal({
  eventTitle,
  onClose,
  onSend,
}: {
  eventTitle: string;
  onClose: () => void;
  onSend: (subject: string, body: string) => Promise<void>;
}) {
  const [subject, setSubject] = useState('');
  const [body, setBody] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (!subject.trim() || !body.trim()) {
      setError('Subject and message are required.');
      return;
    }

    setSending(true);

    try {
      await onSend(subject.trim(), body.trim());
    } catch (sendError) {
      setError(readError(sendError, 'Unable to send announcement.'));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/75 p-4 backdrop-blur-sm">
      <div className="w-full max-w-2xl rounded-2xl border border-border/70 bg-card shadow-[0_24px_80px_rgba(15,23,42,0.22)]">
        <div className="flex items-start justify-between gap-4 border-b border-border/70 p-5">
          <div>
            <h3 className="font-serif text-2xl tracking-tight text-foreground">
              Announcement
            </h3>
            <p className="mt-1 text-sm text-muted-foreground">{eventTitle}</p>
          </div>
          <Button onClick={onClose} size="sm" type="button" variant="outline">
            <X className="h-4 w-4" />
          </Button>
        </div>
        <form className="space-y-4 p-5" onSubmit={handleSubmit}>
          <Input
            maxLength={120}
            onChange={(event) => setSubject(event.target.value)}
            placeholder="Subject"
            value={subject}
          />
          <textarea
            className="min-h-48 w-full rounded-xl border border-input bg-background/90 px-3 py-2 text-sm text-foreground shadow-sm transition placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
            maxLength={4000}
            onChange={(event) => setBody(event.target.value)}
            placeholder="Message"
            value={body}
          />
          {error ? (
            <div className="rounded-xl border border-rose-300/60 bg-rose-500/10 p-3 text-sm text-rose-900 dark:text-rose-100">
              {error}
            </div>
          ) : null}
          <div className="flex flex-wrap justify-end gap-3">
            <Button
              disabled={sending}
              onClick={onClose}
              type="button"
              variant="outline"
            >
              Cancel
            </Button>
            <Button disabled={sending} type="submit">
              {sending ? (
                <LoaderCircle className="h-4 w-4 animate-spin" />
              ) : (
                <Send className="h-4 w-4" />
              )}
              Send
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}

function LoadingTable() {
  return (
    <div className="space-y-3">
      {Array.from({ length: 5 }).map((_, index) => (
        <div
          className="h-14 animate-pulse rounded-xl bg-secondary/50"
          key={index}
        />
      ))}
    </div>
  );
}

function StatusBadge({ status }: { status: RegistrationStatus }) {
  return (
    <span
      className={cn(
        'rounded-full border px-2.5 py-1 text-xs font-medium',
        status === 'registered'
          ? 'border-emerald-300/60 bg-emerald-500/10 text-emerald-800 dark:text-emerald-100'
          : 'border-border/70 bg-secondary/40 text-muted-foreground',
      )}
    >
      {status}
    </span>
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

async function authorizedBlobRequest(
  path: string,
  refreshSession: () => Promise<boolean>,
) {
  const refreshed = await refreshSession();
  const session = useAuthStore.getState().session;

  if (!refreshed || !session) {
    throw new Error('You must be signed in to continue.');
  }

  const response = await fetch(`${appConfig.apiBaseUrl}${path}`, {
    headers: {
      Authorization: `Bearer ${session.accessToken}`,
    },
  });

  if (!response.ok) {
    throw new Error(`Request failed with ${response.status}`);
  }

  return response.blob();
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function readError(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}
