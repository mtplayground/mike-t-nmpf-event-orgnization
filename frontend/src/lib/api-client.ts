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

export type UploadRequestHeader = {
  name: string;
  value: string;
};

export async function uploadFileWithProgress(options: {
  url: string;
  method?: string;
  file: File;
  headers?: UploadRequestHeader[];
  onProgress?: (progress: number) => void;
}) {
  const { file, headers = [], method = 'PUT', onProgress, url } = options;

  await new Promise<void>((resolve, reject) => {
    const request = new XMLHttpRequest();

    request.open(method, url, true);

    for (const header of headers) {
      request.setRequestHeader(header.name, header.value);
    }

    request.upload.addEventListener('progress', (event) => {
      if (!onProgress || !event.lengthComputable) {
        return;
      }

      const progress = Math.round((event.loaded / event.total) * 100);
      onProgress(progress);
    });

    request.addEventListener('load', () => {
      if (request.status >= 200 && request.status < 300) {
        onProgress?.(100);
        resolve();
        return;
      }

      reject(new Error(`Upload failed with ${request.status}`));
    });

    request.addEventListener('error', () => {
      reject(new Error('Upload failed due to a network error'));
    });

    request.send(file);
  });
}

async function safeParseJson(response: Response) {
  try {
    return await response.json();
  } catch {
    return null;
  }
}
