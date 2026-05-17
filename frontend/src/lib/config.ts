const DEFAULT_API_BASE_URL = 'http://127.0.0.1:8080';

function normalizeApiBaseUrl(value?: string) {
  const trimmed = value?.trim();

  if (!trimmed) {
    return DEFAULT_API_BASE_URL;
  }

  return trimmed.replace(/\/+$/, '');
}

export const appConfig = {
  apiBaseUrl: normalizeApiBaseUrl(import.meta.env.VITE_API_BASE_URL),
} as const;
