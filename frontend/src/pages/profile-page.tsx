import {
  useEffect,
  useState,
  type ChangeEvent,
  type FormEvent,
  type ReactNode,
} from 'react';
import { Camera, LoaderCircle, Upload } from 'lucide-react';

import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { type Profile, useAuthStore } from '@/stores/auth-store';

const MAX_AVATAR_SIZE_BYTES = 5 * 1024 * 1024;
const ALLOWED_AVATAR_TYPES = ['image/jpeg', 'image/png', 'image/webp'];

export function ProfilePage() {
  const fetchProfile = useAuthStore((state) => state.fetchProfile);
  const updateProfile = useAuthStore((state) => state.updateProfile);
  const createAvatarUploadUrl = useAuthStore(
    (state) => state.createAvatarUploadUrl,
  );
  const uploadAvatarFile = useAuthStore((state) => state.uploadAvatarFile);
  const confirmAvatarUpload = useAuthStore(
    (state) => state.confirmAvatarUpload,
  );
  const sessionUser = useAuthStore((state) => state.session?.user ?? null);

  const [profile, setProfile] = useState<Profile | null>(null);
  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState(0);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [avatarPreviewUrl, setAvatarPreviewUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadProfile() {
      setLoading(true);
      setError(null);

      try {
        const nextProfile = await fetchProfile();
        if (cancelled) {
          return;
        }

        setProfile(nextProfile);
        setDisplayName(nextProfile.display_name);
        setBio(nextProfile.bio ?? '');
      } catch (loadError) {
        if (!cancelled) {
          setError(readError(loadError, 'Unable to load your profile.'));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadProfile();

    return () => {
      cancelled = true;
    };
  }, [fetchProfile]);

  useEffect(() => {
    return () => {
      if (avatarPreviewUrl) {
        URL.revokeObjectURL(avatarPreviewUrl);
      }
    };
  }, [avatarPreviewUrl]);

  async function handleProfileSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setMessage(null);
    setError(null);

    try {
      const nextProfile = await updateProfile({
        display_name: displayName,
        bio: bio.trim() ? bio : null,
      });

      setProfile(nextProfile);
      setDisplayName(nextProfile.display_name);
      setBio(nextProfile.bio ?? '');
      setMessage('Profile updated.');
    } catch (submitError) {
      setError(readError(submitError, 'Unable to update your profile.'));
    } finally {
      setSaving(false);
    }
  }

  async function handleAvatarChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';

    if (!file) {
      return;
    }

    setMessage(null);
    setError(null);
    if (avatarPreviewUrl) {
      URL.revokeObjectURL(avatarPreviewUrl);
      setAvatarPreviewUrl(null);
    }

    const clientValidationError = validateAvatarFile(file);
    if (clientValidationError) {
      setError(clientValidationError);
      return;
    }

    setUploading(true);
    setUploadProgress(0);

    try {
      const uploadTarget = await createAvatarUploadUrl({
        contentType: file.type,
        sizeBytes: file.size,
      });

      await uploadAvatarFile({
        file,
        headers: uploadTarget.headers,
        method: uploadTarget.method,
        onProgress: setUploadProgress,
        url: uploadTarget.upload_url,
      });

      const nextProfile = await confirmAvatarUpload(uploadTarget.object_key);
      const nextPreviewUrl = URL.createObjectURL(file);
      setProfile(nextProfile);
      setDisplayName(nextProfile.display_name);
      setBio(nextProfile.bio ?? '');
      setAvatarPreviewUrl(nextPreviewUrl);
      setUploadProgress(100);
      setMessage('Avatar updated.');
    } catch (uploadError) {
      setError(readError(uploadError, 'Unable to upload your avatar.'));
      setUploadProgress(0);
    } finally {
      setUploading(false);
    }
  }

  const avatarUrl = avatarPreviewUrl;

  if (loading) {
    return (
      <ProfileShell
        title="Loading your profile"
        description="Fetching your current identity, profile text, and avatar metadata."
      >
        <StatusPanel
          title="Reading account details"
          body="The page waits for your profile record before rendering editable fields."
        />
      </ProfileShell>
    );
  }

  if (!profile) {
    return (
      <ProfileShell
        title="Profile unavailable"
        description="The backend did not return a profile document for this session."
      >
        <StatusPanel
          tone="error"
          title="Profile load failed"
          body={error ?? 'Unable to load your profile.'}
        />
      </ProfileShell>
    );
  }

  return (
    <ProfileShell
      title="Profile and avatar"
      description="Edit the identity details your attendees and hosts will see, then replace your avatar through a constrained presigned upload flow."
    >
      <div className="grid gap-6 xl:grid-cols-[1.15fr_0.85fr]">
        <Card>
          <CardHeader>
            <CardTitle>Profile details</CardTitle>
            <CardDescription>
              Keep your display name polished and your bio brief. Changes are
              saved directly to the authenticated profile record.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form className="space-y-5" onSubmit={handleProfileSubmit}>
              <Field
                label="Email"
                hint="Email is managed by the auth system and shown here as a reference."
              >
                <Input disabled value={profile.email} />
              </Field>
              <Field label="Display name" hint="Between 3 and 64 characters.">
                <Input
                  maxLength={64}
                  onChange={(event) => setDisplayName(event.target.value)}
                  value={displayName}
                />
              </Field>
              <Field label="Bio" hint="Optional, up to 500 characters.">
                <textarea
                  className="min-h-32 w-full rounded-2xl border border-input bg-background/90 px-4 py-3 text-sm text-foreground shadow-sm transition placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  maxLength={500}
                  onChange={(event) => setBio(event.target.value)}
                  placeholder="Tell people what kind of events you run, attend, or care about."
                  value={bio}
                />
              </Field>
              <div className="flex flex-wrap items-center gap-3">
                <Button disabled={saving || uploading} type="submit">
                  {saving ? (
                    <>
                      <LoaderCircle className="h-4 w-4 animate-spin" />
                      Saving
                    </>
                  ) : (
                    'Save profile'
                  )}
                </Button>
                <span className="text-sm text-muted-foreground">
                  Signed in as {sessionUser?.email ?? profile.email}
                </span>
              </div>
            </form>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Avatar uploader</CardTitle>
            <CardDescription>
              Accepts JPEG, PNG, and WebP files up to 5 MB, then uploads
              directly to object storage with progress feedback.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="flex items-center gap-4 rounded-[24px] border border-border/70 bg-secondary/40 p-4">
              <div className="flex h-24 w-24 items-center justify-center overflow-hidden rounded-[22px] border border-border/70 bg-background">
                {avatarUrl ? (
                  <img
                    alt={`${profile.display_name} avatar`}
                    className="h-full w-full object-cover"
                    src={avatarUrl}
                  />
                ) : (
                  <Camera className="h-10 w-10 text-muted-foreground" />
                )}
              </div>
              <div className="space-y-2">
                <p className="text-sm font-medium text-foreground">
                  {profile.display_name}
                </p>
                <p className="text-sm text-muted-foreground">
                  {profile.avatar_object_key
                    ? 'Current avatar is stored and active.'
                    : 'No avatar uploaded yet.'}
                </p>
                <label className="inline-flex cursor-pointer items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-90">
                  <Upload className="h-4 w-4" />
                  {uploading ? 'Uploading...' : 'Choose avatar'}
                  <input
                    accept={ALLOWED_AVATAR_TYPES.join(',')}
                    className="sr-only"
                    disabled={uploading || saving}
                    onChange={handleAvatarChange}
                    type="file"
                  />
                </label>
              </div>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between text-sm text-muted-foreground">
                <span>Upload progress</span>
                <span>{uploading ? `${uploadProgress}%` : 'Idle'}</span>
              </div>
              <div className="h-3 overflow-hidden rounded-full bg-secondary">
                <div
                  className={cn(
                    'h-full rounded-full bg-primary transition-[width] duration-300',
                    uploadProgress === 0 ? 'opacity-40' : 'opacity-100',
                  )}
                  style={{ width: `${uploadProgress}%` }}
                />
              </div>
            </div>

            <div className="rounded-2xl border border-dashed border-border/70 p-4 text-sm leading-6 text-muted-foreground">
              Client-side validation rejects unsupported file types and files
              larger than 5 MB before requesting a presigned URL.
            </div>
          </CardContent>
        </Card>
      </div>

      {message ? (
        <StatusPanel body={message} title="Success" tone="success" />
      ) : null}

      {error ? (
        <StatusPanel
          body={error}
          title="Something needs attention"
          tone="error"
        />
      ) : null}
    </ProfileShell>
  );
}

type ProfileShellProps = {
  title: string;
  description: string;
  children: ReactNode;
};

function ProfileShell({ children, description, title }: ProfileShellProps) {
  return (
    <section className="space-y-6">
      <div className="rounded-[28px] border border-border/60 bg-card/85 px-6 py-6 shadow-[0_24px_80px_rgba(15,23,42,0.12)] backdrop-blur">
        <p className="text-xs font-semibold uppercase tracking-[0.28em] text-muted-foreground">
          Profile
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
  hint: string;
  children: ReactNode;
};

function Field({ children, hint, label }: FieldProps) {
  return (
    <label className="block space-y-2">
      <span className="text-sm font-medium text-foreground">{label}</span>
      {children}
      <span className="block text-xs text-muted-foreground">{hint}</span>
    </label>
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

function validateAvatarFile(file: File) {
  if (!ALLOWED_AVATAR_TYPES.includes(file.type)) {
    return 'Avatar files must be JPEG, PNG, or WebP.';
  }

  if (file.size <= 0) {
    return 'The selected file appears to be empty.';
  }

  if (file.size > MAX_AVATAR_SIZE_BYTES) {
    return 'Avatar files must be 5 MB or smaller.';
  }

  return null;
}

function readError(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallback;
}
