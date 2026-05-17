import { create } from 'zustand';

import {
  requestJson,
  uploadFileWithProgress,
  type UploadRequestHeader,
} from '@/lib/api-client';

const AUTH_STORAGE_KEY = 'event-organization.auth';
const ACCESS_TOKEN_REFRESH_WINDOW_MS = 60_000;

export type AuthUser = {
  id: string;
  email: string;
  display_name: string;
  email_verified: boolean;
  bio?: string | null;
  avatar_object_key?: string | null;
};

type TokenPayload = {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in_seconds: number;
  refresh_expires_in_seconds: number;
  user: AuthUser;
};

export type AuthSession = {
  accessToken: string;
  refreshToken: string;
  tokenType: string;
  accessTokenExpiresAt: number;
  refreshTokenExpiresAt: number;
  user: AuthUser;
};

type RegisterPayload = {
  email: string;
  password: string;
  display_name: string;
};

export type Profile = {
  id: string;
  email: string;
  display_name: string;
  bio: string | null;
  avatar_object_key: string | null;
  email_verified: boolean;
};

type AvatarUploadUrlPayload = {
  object_key: string;
  method: string;
  upload_url: string;
  headers: UploadRequestHeader[];
  expires_in_seconds: number;
  max_size_bytes: number;
};

type AuthStore = {
  initialized: boolean;
  isRefreshing: boolean;
  lastError: string | null;
  session: AuthSession | null;
  clearError: () => void;
  hydrate: () => Promise<void>;
  login: (payload: { email: string; password: string }) => Promise<void>;
  logout: () => Promise<void>;
  refreshSession: (options?: { force?: boolean }) => Promise<boolean>;
  register: (payload: RegisterPayload) => Promise<{
    user_id: string;
    email: string;
    display_name: string;
    email_verification_required: boolean;
  }>;
  verifyEmail: (token: string) => Promise<{
    user_id: string;
    email: string;
    email_verified: boolean;
    verified_at: string | null;
  }>;
  resendVerification: (email: string) => Promise<void>;
  forgotPassword: (email: string) => Promise<void>;
  resetPassword: (token: string, password: string) => Promise<void>;
  fetchProfile: () => Promise<Profile>;
  updateProfile: (payload: {
    display_name?: string;
    bio?: string | null;
  }) => Promise<Profile>;
  createAvatarUploadUrl: (payload: {
    contentType: string;
    sizeBytes: number;
  }) => Promise<AvatarUploadUrlPayload>;
  uploadAvatarFile: (options: {
    url: string;
    method: string;
    headers: UploadRequestHeader[];
    file: File;
    onProgress?: (progress: number) => void;
  }) => Promise<void>;
  confirmAvatarUpload: (objectKey: string) => Promise<Profile>;
};

let inflightRefresh: Promise<boolean> | null = null;

export const useAuthStore = create<AuthStore>((set, get) => ({
  initialized: false,
  isRefreshing: false,
  lastError: null,
  session: null,
  clearError: () => set({ lastError: null }),
  hydrate: async () => {
    const session = readStoredSession();

    if (!session) {
      set({ initialized: true, session: null });
      return;
    }

    set({ initialized: true, session, lastError: null });

    if (session.refreshTokenExpiresAt <= Date.now()) {
      clearStoredSession();
      set({ session: null });
      return;
    }

    await get().refreshSession();
  },
  login: async ({ email, password }) => {
    const data = await requestJson<TokenPayload>('/auth/login', {
      method: 'POST',
      body: { email, password },
    });

    const session = toAuthSession(data);
    persistSession(session);
    set({ session, lastError: null });
  },
  logout: async () => {
    const currentSession = get().session;

    try {
      if (currentSession) {
        await requestJson<{ revoked: boolean }>('/auth/logout', {
          method: 'POST',
          body: { refresh_token: currentSession.refreshToken },
        });
      }
    } catch {
      // Ignore logout API failures and clear the local session.
    }

    clearStoredSession();
    set({ session: null, lastError: null, isRefreshing: false });
  },
  refreshSession: async (options) => {
    const currentSession = get().session;

    if (!currentSession) {
      return false;
    }

    const now = Date.now();

    if (currentSession.refreshTokenExpiresAt <= now) {
      clearStoredSession();
      set({ session: null, isRefreshing: false });
      return false;
    }

    if (
      !options?.force &&
      currentSession.accessTokenExpiresAt - now > ACCESS_TOKEN_REFRESH_WINDOW_MS
    ) {
      return true;
    }

    if (inflightRefresh) {
      return inflightRefresh;
    }

    set({ isRefreshing: true });

    inflightRefresh = requestJson<TokenPayload>('/auth/refresh', {
      method: 'POST',
      body: { refresh_token: currentSession.refreshToken },
    })
      .then((data) => {
        const session = toAuthSession(data);
        persistSession(session);
        set({ session, isRefreshing: false, lastError: null });
        return true;
      })
      .catch(() => {
        clearStoredSession();
        set({
          session: null,
          isRefreshing: false,
          lastError: 'Your session has expired. Please sign in again.',
        });
        return false;
      })
      .finally(() => {
        inflightRefresh = null;
      });

    return inflightRefresh;
  },
  register: async (payload) => {
    return requestJson('/auth/register', {
      method: 'POST',
      body: payload,
    });
  },
  verifyEmail: async (token) => {
    return requestJson('/auth/verify-email', {
      method: 'POST',
      body: { token },
    });
  },
  resendVerification: async (email) => {
    await requestJson('/auth/resend-verification', {
      method: 'POST',
      body: { email },
    });
  },
  forgotPassword: async (email) => {
    await requestJson('/auth/forgot-password', {
      method: 'POST',
      body: { email },
    });
  },
  resetPassword: async (token, password) => {
    await requestJson('/auth/reset-password', {
      method: 'POST',
      body: { token, password },
    });
  },
  fetchProfile: async (): Promise<Profile> => {
    const refreshed = await get().refreshSession();
    const session = get().session;

    if (!refreshed || !session) {
      throw new Error('You must be signed in to continue.');
    }

    return requestJson<Profile>('/me', {
      token: session.accessToken,
    });
  },
  updateProfile: async (payload): Promise<Profile> => {
    const refreshed = await get().refreshSession();
    const session = get().session;

    if (!refreshed || !session) {
      throw new Error('You must be signed in to continue.');
    }

    const profile = await requestJson<Profile>('/me', {
      method: 'PATCH',
      body: payload,
      token: session.accessToken,
    });

    syncSessionUser(set, get, profile);
    return profile;
  },
  createAvatarUploadUrl: async ({
    contentType,
    sizeBytes,
  }): Promise<AvatarUploadUrlPayload> => {
    const refreshed = await get().refreshSession();
    const session = get().session;

    if (!refreshed || !session) {
      throw new Error('You must be signed in to continue.');
    }

    return requestJson<AvatarUploadUrlPayload>('/me/avatar/upload-url', {
      method: 'POST',
      body: {
        content_type: contentType,
        size_bytes: sizeBytes,
      },
      token: session.accessToken,
    });
  },
  uploadAvatarFile: async ({
    file,
    headers,
    method,
    onProgress,
    url,
  }): Promise<void> => {
    await uploadFileWithProgress({
      file,
      headers,
      method,
      onProgress,
      url,
    });
  },
  confirmAvatarUpload: async (objectKey): Promise<Profile> => {
    const refreshed = await get().refreshSession();
    const session = get().session;

    if (!refreshed || !session) {
      throw new Error('You must be signed in to continue.');
    }

    const profile = await requestJson<Profile>('/me/avatar/confirm', {
      method: 'POST',
      body: { object_key: objectKey },
      token: session.accessToken,
    });

    syncSessionUser(set, get, profile);
    return profile;
  },
}));

function toAuthSession(payload: TokenPayload): AuthSession {
  const now = Date.now();

  return {
    accessToken: payload.access_token,
    refreshToken: payload.refresh_token,
    tokenType: payload.token_type,
    accessTokenExpiresAt: now + payload.expires_in_seconds * 1000,
    refreshTokenExpiresAt: now + payload.refresh_expires_in_seconds * 1000,
    user: payload.user,
  };
}

function persistSession(session: AuthSession) {
  window.localStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify(session));
}

function readStoredSession(): AuthSession | null {
  try {
    const raw = window.localStorage.getItem(AUTH_STORAGE_KEY);
    if (!raw) {
      return null;
    }

    return JSON.parse(raw) as AuthSession;
  } catch {
    return null;
  }
}

function clearStoredSession() {
  window.localStorage.removeItem(AUTH_STORAGE_KEY);
}

function syncSessionUser(
  set: (partial: Partial<AuthStore>) => void,
  get: () => AuthStore,
  profile: Profile,
) {
  const session = get().session;

  if (!session) {
    return;
  }

  const nextSession: AuthSession = {
    ...session,
    user: {
      ...session.user,
      display_name: profile.display_name,
      bio: profile.bio,
      avatar_object_key: profile.avatar_object_key,
      email_verified: profile.email_verified,
    },
  };

  persistSession(nextSession);
  set({ session: nextSession });
}
