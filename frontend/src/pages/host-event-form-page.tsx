import {
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type FormEvent,
  type ReactNode,
} from 'react';
import {
  CalendarClock,
  ImagePlus,
  LoaderCircle,
  Save,
  Upload,
} from 'lucide-react';
import { Link, useNavigate, useParams } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import {
  requestJson,
  uploadFileWithProgress,
  type UploadRequestHeader,
} from '@/lib/api-client';
import { cn } from '@/lib/utils';
import { useAuthStore } from '@/stores/auth-store';

const MAX_COVER_SIZE_BYTES = 10 * 1024 * 1024;
const ALLOWED_COVER_TYPES = ['image/jpeg', 'image/png', 'image/webp'];

type HostEventFormPageProps = {
  mode: 'create' | 'edit';
};

type EventLocationType = 'in_person' | 'virtual' | 'hybrid';
type EventVisibility = 'draft' | 'public' | 'unlisted' | 'private';
type EventStatus = 'draft' | 'published' | 'cancelled' | 'completed';

type EventResponse = {
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
  cover_image_id: string | null;
};

type EventCoverUploadUrlResponse = {
  object_key: string;
  method: string;
  upload_url: string;
  headers: UploadRequestHeader[];
  max_size_bytes: number;
};

type EventFormState = {
  title: string;
  descriptionMd: string;
  startAt: string;
  endAt: string;
  timezone: string;
  locationType: EventLocationType;
  locationText: string;
  locationUrl: string;
  capacity: string;
  visibility: EventVisibility;
};

type CoverSelection = {
  file: File;
  previewUrl: string;
  width: number;
  height: number;
};

type FieldErrors = Partial<Record<keyof EventFormState | 'cover', string>>;

const defaultTimezone =
  Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';

export function HostEventFormPage({ mode }: HostEventFormPageProps) {
  const navigate = useNavigate();
  const { eventId } = useParams();
  const refreshSession = useAuthStore((state) => state.refreshSession);

  const [form, setForm] = useState<EventFormState>(() => buildInitialForm());
  const [existingEvent, setExistingEvent] = useState<EventResponse | null>(
    null,
  );
  const [coverSelection, setCoverSelection] = useState<CoverSelection | null>(
    null,
  );
  const [coverProgress, setCoverProgress] = useState(0);
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>({});
  const [loading, setLoading] = useState(mode === 'edit');
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pageTitle = mode === 'create' ? 'Create event' : 'Edit event';
  const submitLabel = mode === 'create' ? 'Save event' : 'Update event';

  const coverName = useMemo(() => {
    if (coverSelection) {
      return coverSelection.file.name;
    }

    if (existingEvent?.cover_image_id) {
      return 'Stored cover image';
    }

    return 'No cover selected';
  }, [coverSelection, existingEvent]);

  useEffect(() => {
    if (mode !== 'edit') {
      return;
    }

    let cancelled = false;

    async function loadEvent() {
      if (!eventId) {
        setError('Missing event id.');
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);

      try {
        const event = await authorizedRequest<EventResponse>(
          `/events/${eventId}`,
          refreshSession,
        );

        if (cancelled) {
          return;
        }

        setExistingEvent(event);
        setForm(formFromEvent(event));
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
  }, [eventId, mode, refreshSession]);

  useEffect(() => {
    return () => {
      if (coverSelection) {
        URL.revokeObjectURL(coverSelection.previewUrl);
      }
    };
  }, [coverSelection]);

  function updateField<K extends keyof EventFormState>(
    field: K,
    value: EventFormState[K],
  ) {
    setForm((current) => ({ ...current, [field]: value }));
    setFieldErrors((current) => ({ ...current, [field]: undefined }));
  }

  async function handleCoverChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';

    if (!file) {
      return;
    }

    setMessage(null);
    setError(null);
    setFieldErrors((current) => ({ ...current, cover: undefined }));

    const validationError = validateCoverFile(file);
    if (validationError) {
      setFieldErrors((current) => ({ ...current, cover: validationError }));
      return;
    }

    try {
      const dimensions = await readImageDimensions(file);
      const previewUrl = URL.createObjectURL(file);

      setCoverSelection((current) => {
        if (current) {
          URL.revokeObjectURL(current.previewUrl);
        }

        return { file, previewUrl, ...dimensions };
      });
      setCoverProgress(0);
    } catch {
      setFieldErrors((current) => ({
        ...current,
        cover: 'The selected file could not be read as an image.',
      }));
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await saveEvent(form.visibility);
  }

  async function handleDraftSave() {
    await saveEvent('draft');
  }

  async function saveEvent(visibility: EventVisibility) {
    const nextForm = { ...form, visibility };
    const nextErrors = validateEventForm(nextForm, coverSelection);
    setFieldErrors(nextErrors);
    setMessage(null);
    setError(null);

    if (Object.keys(nextErrors).length > 0) {
      return;
    }

    setSaving(true);

    try {
      const payload = buildEventPayload(nextForm);
      const event =
        mode === 'edit' && eventId
          ? await authorizedRequest<EventResponse>(
              `/events/${eventId}`,
              refreshSession,
              {
                method: 'PATCH',
                body: payload,
              },
            )
          : await authorizedRequest<EventResponse>('/events', refreshSession, {
              method: 'POST',
              body: payload,
            });

      let savedEvent = event;

      if (coverSelection) {
        await uploadCover(
          event.id,
          coverSelection,
          refreshSession,
          setCoverProgress,
        );
        savedEvent = await authorizedRequest<EventResponse>(
          `/events/${event.id}`,
          refreshSession,
        );
      }

      setExistingEvent(savedEvent);
      setForm(formFromEvent(savedEvent));
      setCoverSelection((current) => {
        if (current) {
          URL.revokeObjectURL(current.previewUrl);
        }

        return null;
      });
      setCoverProgress(coverSelection ? 100 : 0);
      setMessage(visibility === 'draft' ? 'Draft saved.' : 'Event saved.');

      if (mode === 'create') {
        navigate(`/host/events/${event.id}/edit`, { replace: true });
      }
    } catch (saveError) {
      setError(readError(saveError, 'Unable to save this event.'));
      setCoverProgress(0);
    } finally {
      setSaving(false);
    }
  }

  if (loading) {
    return (
      <EventFormShell
        title="Loading event"
        description="Fetching the event record before opening editable fields."
      >
        <StatusPanel body="Reading host-owned event data." title="Loading" />
      </EventFormShell>
    );
  }

  return (
    <EventFormShell
      title={pageTitle}
      description="Manage event details, schedule, location, visibility, and cover artwork from one host-owned form."
    >
      <form className="space-y-6" onSubmit={handleSubmit}>
        <div className="grid gap-6 xl:grid-cols-[1.25fr_0.75fr]">
          <Card>
            <CardHeader>
              <CardTitle>Event details</CardTitle>
              <CardDescription>
                Core copy, schedule, timezone, and publication state.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-5">
              <Field error={fieldErrors.title} label="Title">
                <Input
                  maxLength={180}
                  onChange={(event) => updateField('title', event.target.value)}
                  placeholder="Spring community meetup"
                  value={form.title}
                />
              </Field>

              <Field error={fieldErrors.descriptionMd} label="Description">
                <textarea
                  className="min-h-40 w-full rounded-xl border border-input bg-background/90 px-3 py-3 text-sm text-foreground shadow-sm transition placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  maxLength={20_000}
                  onChange={(event) =>
                    updateField('descriptionMd', event.target.value)
                  }
                  placeholder="Share the event agenda, notes, and links in markdown."
                  value={form.descriptionMd}
                />
              </Field>

              <div className="grid gap-4 md:grid-cols-2">
                <Field error={fieldErrors.startAt} label="Starts">
                  <Input
                    onChange={(event) =>
                      updateField('startAt', event.target.value)
                    }
                    type="datetime-local"
                    value={form.startAt}
                  />
                </Field>
                <Field error={fieldErrors.endAt} label="Ends">
                  <Input
                    onChange={(event) =>
                      updateField('endAt', event.target.value)
                    }
                    type="datetime-local"
                    value={form.endAt}
                  />
                </Field>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <Field error={fieldErrors.timezone} label="Timezone">
                  <Input
                    maxLength={100}
                    onChange={(event) =>
                      updateField('timezone', event.target.value)
                    }
                    value={form.timezone}
                  />
                </Field>
                <Field error={fieldErrors.capacity} label="Capacity">
                  <Input
                    min={1}
                    onChange={(event) =>
                      updateField('capacity', event.target.value)
                    }
                    placeholder="Optional"
                    type="number"
                    value={form.capacity}
                  />
                </Field>
              </div>

              <Field error={fieldErrors.visibility} label="Visibility">
                <select
                  className="h-11 w-full rounded-xl border border-input bg-background/90 px-3 text-sm text-foreground shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onChange={(event) =>
                    updateField(
                      'visibility',
                      event.target.value as EventVisibility,
                    )
                  }
                  value={form.visibility}
                >
                  <option value="draft">Draft</option>
                  <option value="public">Public</option>
                  <option value="unlisted">Unlisted</option>
                  <option value="private">Private</option>
                </select>
              </Field>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Cover image</CardTitle>
              <CardDescription>
                JPEG, PNG, or WebP up to 10 MB. The API creates hero and
                thumbnail variants after upload.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-5">
              <div className="overflow-hidden rounded-2xl border border-border/70 bg-secondary/30">
                <div className="flex aspect-[16/9] items-center justify-center bg-background">
                  {coverSelection ? (
                    <img
                      alt="Selected event cover"
                      className="h-full w-full object-cover"
                      src={coverSelection.previewUrl}
                    />
                  ) : (
                    <ImagePlus className="h-12 w-12 text-muted-foreground" />
                  )}
                </div>
                <div className="space-y-3 p-4">
                  <p className="truncate text-sm font-medium text-foreground">
                    {coverName}
                  </p>
                  <label className="inline-flex cursor-pointer items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90">
                    <Upload className="h-4 w-4" />
                    Choose cover
                    <input
                      accept={ALLOWED_COVER_TYPES.join(',')}
                      className="sr-only"
                      disabled={saving}
                      onChange={handleCoverChange}
                      type="file"
                    />
                  </label>
                </div>
              </div>

              <ProgressBar progress={coverProgress} />
              {fieldErrors.cover ? (
                <p className="text-sm text-rose-700 dark:text-rose-200">
                  {fieldErrors.cover}
                </p>
              ) : null}
            </CardContent>
          </Card>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Location</CardTitle>
            <CardDescription>
              Match the required location fields to the event format.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <Field error={fieldErrors.locationType} label="Location type">
              <select
                className="h-11 w-full rounded-xl border border-input bg-background/90 px-3 text-sm text-foreground shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onChange={(event) =>
                  updateField(
                    'locationType',
                    event.target.value as EventLocationType,
                  )
                }
                value={form.locationType}
              >
                <option value="in_person">In person</option>
                <option value="virtual">Virtual</option>
                <option value="hybrid">Hybrid</option>
              </select>
            </Field>

            <div className="grid gap-4 md:grid-cols-2">
              <Field error={fieldErrors.locationText} label="Venue or address">
                <Input
                  maxLength={500}
                  onChange={(event) =>
                    updateField('locationText', event.target.value)
                  }
                  placeholder="Main hall, 123 Market Street"
                  value={form.locationText}
                />
              </Field>
              <Field error={fieldErrors.locationUrl} label="Virtual URL">
                <Input
                  maxLength={1000}
                  onChange={(event) =>
                    updateField('locationUrl', event.target.value)
                  }
                  placeholder="https://example.com/room"
                  type="url"
                  value={form.locationUrl}
                />
              </Field>
            </div>
          </CardContent>
        </Card>

        <div className="flex flex-wrap items-center gap-3">
          <Button disabled={saving} type="submit">
            {saving ? (
              <LoaderCircle className="h-4 w-4 animate-spin" />
            ) : (
              <Save className="h-4 w-4" />
            )}
            {submitLabel}
          </Button>
          <Button
            disabled={saving}
            onClick={handleDraftSave}
            type="button"
            variant="outline"
          >
            <CalendarClock className="h-4 w-4" />
            Save draft
          </Button>
          <Button asChild variant="ghost">
            <Link to="/host">Back to host</Link>
          </Button>
        </div>
      </form>

      {message ? (
        <StatusPanel body={message} title="Saved" tone="success" />
      ) : null}
      {error ? (
        <StatusPanel
          body={error}
          title="Something needs attention"
          tone="error"
        />
      ) : null}
    </EventFormShell>
  );
}

function buildInitialForm(): EventFormState {
  const now = new Date();
  now.setMinutes(now.getMinutes() + 60);
  const later = new Date(now);
  later.setHours(later.getHours() + 2);

  return {
    title: '',
    descriptionMd: '',
    startAt: toDateTimeLocalValue(now.toISOString()),
    endAt: toDateTimeLocalValue(later.toISOString()),
    timezone: defaultTimezone,
    locationType: 'in_person',
    locationText: '',
    locationUrl: '',
    capacity: '',
    visibility: 'draft',
  };
}

function formFromEvent(event: EventResponse): EventFormState {
  return {
    title: event.title,
    descriptionMd: event.description_md,
    startAt: toDateTimeLocalValue(event.start_at),
    endAt: toDateTimeLocalValue(event.end_at),
    timezone: event.timezone,
    locationType: event.location_type,
    locationText: event.location_text ?? '',
    locationUrl: event.location_url ?? '',
    capacity: event.capacity ? String(event.capacity) : '',
    visibility: event.visibility,
  };
}

function buildEventPayload(form: EventFormState) {
  const visibility = form.visibility;

  return {
    title: form.title.trim(),
    description_md: form.descriptionMd.trim(),
    start_at: new Date(form.startAt).toISOString(),
    end_at: new Date(form.endAt).toISOString(),
    timezone: form.timezone.trim(),
    location_type: form.locationType,
    location_text: form.locationText.trim() || null,
    location_url: form.locationUrl.trim() || null,
    capacity: form.capacity.trim() ? Number(form.capacity) : null,
    visibility,
    status: visibility === 'draft' ? 'draft' : 'published',
  };
}

function validateEventForm(
  form: EventFormState,
  coverSelection: CoverSelection | null,
) {
  const errors: FieldErrors = {};
  const title = form.title.trim();
  const timezone = form.timezone.trim();
  const start = new Date(form.startAt);
  const end = new Date(form.endAt);
  const capacity = form.capacity.trim() ? Number(form.capacity) : null;

  if (!title) {
    errors.title = 'Title is required.';
  } else if (title.length > 180) {
    errors.title = 'Title must be 180 characters or fewer.';
  }

  if (form.descriptionMd.length > 20_000) {
    errors.descriptionMd = 'Description must be 20000 characters or fewer.';
  }

  if (!form.startAt || Number.isNaN(start.getTime())) {
    errors.startAt = 'Start date and time are required.';
  }

  if (!form.endAt || Number.isNaN(end.getTime())) {
    errors.endAt = 'End date and time are required.';
  } else if (!Number.isNaN(start.getTime()) && end < start) {
    errors.endAt = 'End time must be at or after the start time.';
  }

  if (!timezone) {
    errors.timezone = 'Timezone is required.';
  } else if (timezone.length > 100) {
    errors.timezone = 'Timezone must be 100 characters or fewer.';
  }

  if (capacity !== null && (!Number.isInteger(capacity) || capacity <= 0)) {
    errors.capacity = 'Capacity must be a positive whole number.';
  }

  if (form.locationType === 'in_person' && !form.locationText.trim()) {
    errors.locationText = 'Venue or address is required for in-person events.';
  }

  if (form.locationType === 'virtual' && !form.locationUrl.trim()) {
    errors.locationUrl = 'Virtual URL is required for virtual events.';
  }

  if (form.locationType === 'hybrid') {
    if (!form.locationText.trim()) {
      errors.locationText = 'Venue or address is required for hybrid events.';
    }
    if (!form.locationUrl.trim()) {
      errors.locationUrl = 'Virtual URL is required for hybrid events.';
    }
  }

  if (coverSelection) {
    const coverError = validateCoverFile(coverSelection.file);
    if (coverError) {
      errors.cover = coverError;
    }
  }

  return errors;
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

async function uploadCover(
  eventId: string,
  coverSelection: CoverSelection,
  refreshSession: () => Promise<boolean>,
  onProgress: (progress: number) => void,
) {
  const uploadTarget = await authorizedRequest<EventCoverUploadUrlResponse>(
    `/events/${eventId}/cover/upload-url`,
    refreshSession,
    {
      method: 'POST',
      body: {
        content_type: coverSelection.file.type,
        size_bytes: coverSelection.file.size,
      },
    },
  );

  await uploadFileWithProgress({
    file: coverSelection.file,
    headers: uploadTarget.headers,
    method: uploadTarget.method,
    onProgress,
    url: uploadTarget.upload_url,
  });

  await authorizedRequest(`/events/${eventId}/cover/confirm`, refreshSession, {
    method: 'POST',
    body: {
      object_key: uploadTarget.object_key,
      width: coverSelection.width,
      height: coverSelection.height,
    },
  });
}

function validateCoverFile(file: File) {
  if (!ALLOWED_COVER_TYPES.includes(file.type)) {
    return 'Cover images must be JPEG, PNG, or WebP.';
  }

  if (file.size <= 0) {
    return 'The selected cover file appears to be empty.';
  }

  if (file.size > MAX_COVER_SIZE_BYTES) {
    return 'Cover images must be 10 MB or smaller.';
  }

  return null;
}

async function readImageDimensions(file: File) {
  const imageUrl = URL.createObjectURL(file);

  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const nextImage = new Image();
      nextImage.onload = () => resolve(nextImage);
      nextImage.onerror = () => reject(new Error('image load failed'));
      nextImage.src = imageUrl;
    });

    return {
      width: image.naturalWidth,
      height: image.naturalHeight,
    };
  } finally {
    URL.revokeObjectURL(imageUrl);
  }
}

function toDateTimeLocalValue(value: string) {
  const date = new Date(value);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');

  return `${year}-${month}-${day}T${hours}:${minutes}`;
}

function readError(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallback;
}

type EventFormShellProps = {
  title: string;
  description: string;
  children: ReactNode;
};

function EventFormShell({ children, description, title }: EventFormShellProps) {
  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
          Host event
        </p>
        <h2 className="mt-3 font-serif text-3xl tracking-tight text-foreground">
          {title}
        </h2>
        <p className="mt-3 max-w-3xl text-sm leading-7 text-muted-foreground">
          {description}
        </p>
      </div>
      {children}
    </section>
  );
}

type FieldProps = {
  label: string;
  error?: string;
  children: ReactNode;
};

function Field({ children, error, label }: FieldProps) {
  return (
    <label className="block space-y-2">
      <span className="text-sm font-medium text-foreground">{label}</span>
      {children}
      {error ? (
        <span className="block text-xs text-rose-700 dark:text-rose-200">
          {error}
        </span>
      ) : null}
    </label>
  );
}

function ProgressBar({ progress }: { progress: number }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>Upload progress</span>
        <span>{progress > 0 ? `${progress}%` : 'Idle'}</span>
      </div>
      <div className="h-3 overflow-hidden rounded-full bg-secondary">
        <div
          className={cn(
            'h-full rounded-full bg-primary transition-[width] duration-300',
            progress === 0 ? 'opacity-40' : 'opacity-100',
          )}
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
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
