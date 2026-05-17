import { appConfig } from '@/lib/config';

type ApiEnvelope<T> = {
  data: T;
};

type ApiErrorEnvelope = {
  error?: {
    message?: string;
  };
};

type RequestOptions = Omit<RequestInit, 'body'> & {
  body?: unknown;
  token?: string | null;
};

export async function requestJson<T>(
  path: string,
  { body, headers, token, ...init }: RequestOptions = {},
) {
  const response = await fetch(`${appConfig.apiBaseUrl}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...headers,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (!response.ok) {
    const payload = (await safeParseJson(response)) as ApiErrorEnvelope | null;
    throw new Error(
      payload?.error?.message || `Request failed with ${response.status}`,
    );
  }

  const payload = (await response.json()) as ApiEnvelope<T>;
  return payload.data;
}

async function safeParseJson(response: Response) {
  try {
    return await response.json();
  } catch {
    return null;
  }
}
