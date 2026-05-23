import { expect, type Page, type Route, test } from '@playwright/test';

const apiBaseUrl = `http://127.0.0.1:${process.env.PLAYWRIGHT_API_PORT ?? '4174'}`;
const eventId = 'event-e2e-1';
const eventSlug = 'spring-community-showcase';
const eventTitle = 'Spring Community Showcase';
const startAt = '2030-04-19T15:00:00.000Z';
const endAt = '2030-04-19T17:00:00.000Z';
const coverDataUrl =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=';
const pngBytes = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=',
  'base64',
);

type UserRecord = {
  id: string;
  email: string;
  displayName: string;
  emailVerified: boolean;
};

type EventRecord = {
  id: string;
  host_id: string;
  title: string;
  slug: string;
  description_md: string;
  start_at: string;
  end_at: string;
  timezone: string;
  location_type: 'in_person' | 'virtual' | 'hybrid';
  location_text: string | null;
  location_url: string | null;
  capacity: number | null;
  visibility: 'draft' | 'public' | 'unlisted' | 'private';
  status: 'draft' | 'published' | 'cancelled' | 'completed';
  cover_image_id: string | null;
  cancelled_at: string | null;
};

type RegistrationRecord = {
  id: string;
  event_id: string;
  user_id: string;
  status: 'registered' | 'cancelled';
  registered_at: string;
  cancelled_at: string | null;
};

type MockState = {
  users: Map<string, UserRecord>;
  event: EventRecord | null;
  registration: RegistrationRecord | null;
  csvExports: number;
  announcements: Array<{ subject: string; body: string }>;
};

test('register, verify, publish, register attendee, export, announce, and cancel', async ({
  page,
}) => {
  const state = createMockState();
  await installApiMock(page, state);

  await registerAndVerify(page, {
    displayName: 'Host Organizer',
    email: 'host@example.com',
  });
  await login(page, 'host@example.com');

  await page.getByRole('link', { name: 'New event' }).click();
  await expect(
    page.getByRole('heading', { name: 'Create event' }),
  ).toBeVisible();

  await page.getByLabel('Title').fill(eventTitle);
  await page
    .getByLabel('Description')
    .fill(
      'A public event used to validate the full organizer and attendee flow.',
    );
  await page.getByLabel('Starts').fill('2030-04-19T15:00');
  await page.getByLabel('Ends').fill('2030-04-19T17:00');
  await page.getByLabel('Timezone').fill('America/New_York');
  await page.getByLabel('Capacity').fill('40');
  await page.getByLabel('Visibility').selectOption('public');
  await page
    .getByLabel('Venue or address')
    .fill('Main Hall, 123 Market Street');
  await page.getByLabel('Choose cover').setInputFiles({
    name: 'cover.png',
    mimeType: 'image/png',
    buffer: pngBytes,
  });
  await expect(page.getByText('cover.png')).toBeVisible();

  await page.getByRole('button', { name: 'Save event' }).click();
  await expect(page.getByText('Event saved.')).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/host/events/${eventId}/edit$`));

  await page.getByRole('button', { name: 'Sign out' }).click();
  await registerAndVerify(page, {
    displayName: 'Second Attendee',
    email: 'attendee@example.com',
  });
  await login(page, 'attendee@example.com');

  await page.goto(`/events/${eventSlug}`);
  await expect(page.getByRole('heading', { name: eventTitle })).toBeVisible();
  await page.getByRole('button', { name: 'Register' }).click();
  await expect(page.getByText('Registration confirmed.')).toBeVisible();
  await expect(page.getByText('You are registered')).toBeVisible();

  await page.getByRole('button', { name: 'Sign out' }).click();
  await login(page, 'host@example.com');
  await page.goto(`/host/events/${eventId}/attendees`);
  await expect(page.getByText('Second Attendee')).toBeVisible();

  await page.getByRole('button', { name: 'CSV' }).click();
  await expect.poll(() => state.csvExports).toBe(1);

  await page.getByRole('button', { name: 'Announce' }).click();
  await page.getByPlaceholder('Subject').fill('Event update');
  await page.getByPlaceholder('Message').fill('Doors open 15 minutes early.');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(
    page.getByText('Announcement queued for 1 attendee.'),
  ).toBeVisible();
  expect(state.announcements).toEqual([
    {
      subject: 'Event update',
      body: 'Doors open 15 minutes early.',
    },
  ]);

  await page.getByRole('button', { name: 'Sign out' }).click();
  await login(page, 'attendee@example.com');
  await page.goto('/attendee');
  const registrationCard = page
    .locator('article')
    .filter({ hasText: eventTitle })
    .first();
  await expect(registrationCard).toBeVisible();
  await Promise.all([
    page.waitForResponse(
      (response) =>
        response.url() === `${apiBaseUrl}/events/${eventId}/register` &&
        response.request().method() === 'DELETE',
    ),
    registrationCard.getByRole('button', { name: /^Cancel$/ }).click(),
  ]);
  await expect.poll(() => state.registration?.status).toBe('cancelled');
  await page.reload();
  await expect(page.getByText('0 upcoming registrations')).toBeVisible();
  await expect(
    page.getByText('No upcoming registered events yet.'),
  ).toBeVisible();
});

async function registerAndVerify(
  page: Page,
  user: { displayName: string; email: string },
) {
  await page.goto('/auth/register');
  await page.getByLabel('Display name').fill(user.displayName);
  await page.getByLabel('Email').fill(user.email);
  await page.getByLabel('Password').fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Create account' }).click();

  await expect(
    page.getByRole('heading', { name: 'Confirm the email token' }),
  ).toBeVisible();
  await page.getByLabel('Verification token').fill(`verify:${user.email}`);
  await page.getByRole('button', { name: 'Verify email' }).click();
  await expect(
    page.getByText('Email verified. You can sign in now.'),
  ).toBeVisible();
}

async function login(page: Page, email: string) {
  await page.goto('/auth/login');
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(
    page.getByText(`Signed in as ${displayNameFor(email)}`),
  ).toBeVisible();
}

async function installApiMock(page: Page, state: MockState) {
  await page.route(`${apiBaseUrl}/**`, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = `${url.pathname}${url.search}`;
    const method = request.method();

    if (method === 'PUT' && url.pathname.startsWith('/uploads/')) {
      await route.fulfill({ status: 204 });
      return;
    }

    if (method === 'POST' && url.pathname === '/auth/register') {
      const payload = postJson<{ email: string; display_name: string }>(route);
      const user = {
        id: `user-${state.users.size + 1}`,
        email: payload.email,
        displayName: payload.display_name,
        emailVerified: false,
      };
      state.users.set(payload.email, user);

      await fulfillJson(route, {
        user_id: user.id,
        email: user.email,
        display_name: user.displayName,
        email_verification_required: true,
      });
      return;
    }

    if (method === 'POST' && url.pathname === '/auth/verify-email') {
      const payload = postJson<{ token: string }>(route);
      const email = payload.token.replace(/^verify:/, '');
      const user = requireUser(state, email);
      user.emailVerified = true;

      await fulfillJson(route, {
        user_id: user.id,
        email: user.email,
        email_verified: true,
        verified_at: new Date().toISOString(),
      });
      return;
    }

    if (method === 'POST' && url.pathname === '/auth/login') {
      const payload = postJson<{ email: string }>(route);
      const user = requireUser(state, payload.email);
      await fulfillJson(route, tokenPayload(user));
      return;
    }

    if (method === 'POST' && url.pathname === '/auth/refresh') {
      const payload = postJson<{ refresh_token: string }>(route);
      const user = requireUser(
        state,
        payload.refresh_token.replace(/^refresh:/, ''),
      );
      await fulfillJson(route, tokenPayload(user));
      return;
    }

    if (method === 'POST' && url.pathname === '/auth/logout') {
      await fulfillJson(route, { revoked: true });
      return;
    }

    if (method === 'GET' && path.startsWith('/me/events?')) {
      await fulfillJson(route, hostEventList(state));
      return;
    }

    if (method === 'POST' && url.pathname === '/events') {
      const payload = postJson<Partial<EventRecord>>(route);
      const host = userFromAuthHeader(state, request.headers().authorization);
      state.event = {
        id: eventId,
        host_id: host.id,
        title: payload.title ?? eventTitle,
        slug: eventSlug,
        description_md: payload.description_md ?? '',
        start_at: payload.start_at ?? startAt,
        end_at: payload.end_at ?? endAt,
        timezone: payload.timezone ?? 'UTC',
        location_type: payload.location_type ?? 'in_person',
        location_text: payload.location_text ?? null,
        location_url: payload.location_url ?? null,
        capacity: payload.capacity ?? null,
        visibility: payload.visibility ?? 'public',
        status: payload.status ?? 'published',
        cover_image_id: null,
        cancelled_at: null,
      };
      await fulfillJson(route, state.event);
      return;
    }

    if (method === 'GET' && url.pathname === `/me/events/${eventId}`) {
      await fulfillJson(route, requireEvent(state));
      return;
    }

    if (
      method === 'POST' &&
      url.pathname === `/events/${eventId}/cover/upload-url`
    ) {
      await fulfillJson(route, {
        object_key: `events/${eventId}/cover/original.png`,
        method: 'PUT',
        upload_url: `${apiBaseUrl}/uploads/${eventId}/cover.png`,
        headers: [],
        max_size_bytes: 10 * 1024 * 1024,
      });
      return;
    }

    if (
      method === 'POST' &&
      url.pathname === `/events/${eventId}/cover/confirm`
    ) {
      requireEvent(state).cover_image_id = 'cover-image-1';
      await fulfillJson(route, {
        event_id: eventId,
        hero: {
          object_key: `events/${eventId}/cover/hero.png`,
          public_url: coverDataUrl,
          width: 1600,
          height: 900,
          bytes: 1234,
        },
        thumbnail: {
          object_key: `events/${eventId}/cover/thumb.png`,
          public_url: coverDataUrl,
          width: 480,
          height: 270,
          bytes: 456,
        },
      });
      return;
    }

    if (method === 'GET' && url.pathname === `/public/events/${eventSlug}`) {
      const user = optionalUserFromAuthHeader(
        state,
        request.headers().authorization,
      );
      await fulfillJson(route, publicEventDetail(state, user));
      return;
    }

    if (method === 'GET' && url.pathname === '/events') {
      await fulfillJson(route, publicEventList(state));
      return;
    }

    if (method === 'POST' && url.pathname === `/events/${eventId}/register`) {
      const attendee = userFromAuthHeader(
        state,
        request.headers().authorization,
      );
      state.registration = {
        id: 'registration-1',
        event_id: eventId,
        user_id: attendee.id,
        status: 'registered',
        registered_at: new Date().toISOString(),
        cancelled_at: null,
      };
      await fulfillJson(route, state.registration);
      return;
    }

    if (method === 'DELETE' && url.pathname === `/events/${eventId}/register`) {
      const registration = requireRegistration(state);
      registration.status = 'cancelled';
      registration.cancelled_at = new Date().toISOString();
      await fulfillJson(route, registration);
      return;
    }

    if (method === 'GET' && url.pathname === `/events/${eventId}/attendees`) {
      await fulfillJson(route, hostAttendees(state));
      return;
    }

    if (
      method === 'GET' &&
      url.pathname === `/events/${eventId}/attendees.csv`
    ) {
      state.csvExports += 1;
      await route.fulfill({
        status: 200,
        contentType: 'text/csv',
        body: 'display_name,email,status\nSecond Attendee,attendee@example.com,registered\n',
      });
      return;
    }

    if (method === 'POST' && url.pathname === `/events/${eventId}/announce`) {
      const payload = postJson<{ subject: string; body: string }>(route);
      state.announcements.push(payload);
      await fulfillJson(route, {
        accepted: true,
        recipient_count: 1,
      });
      return;
    }

    if (method === 'GET' && path.startsWith('/me/registrations?')) {
      await fulfillJson(route, attendeeRegistrations(state));
      return;
    }

    await route.fulfill({
      status: 404,
      json: {
        error: {
          code: 'not_found',
          message: `Unhandled mock route: ${method} ${path}`,
        },
      },
    });
  });
}

function createMockState(): MockState {
  return {
    users: new Map(),
    event: null,
    registration: null,
    csvExports: 0,
    announcements: [],
  };
}

function postJson<T>(route: Route): T {
  return route.request().postDataJSON() as T;
}

async function fulfillJson(route: Route, data: unknown) {
  await route.fulfill({
    status: 200,
    contentType: 'application/json',
    json: { data },
  });
}

function tokenPayload(user: UserRecord) {
  return {
    access_token: `access:${user.email}`,
    refresh_token: `refresh:${user.email}`,
    token_type: 'Bearer',
    expires_in_seconds: 3600,
    refresh_expires_in_seconds: 86_400,
    user: userPayload(user),
  };
}

function userPayload(user: UserRecord) {
  return {
    id: user.id,
    email: user.email,
    display_name: user.displayName,
    email_verified: user.emailVerified,
    bio: null,
    avatar_object_key: null,
  };
}

function hostEventList(state: MockState) {
  const event = state.event;
  const items = event
    ? [{ ...event, attendee_count: activeAttendeeCount(state) }]
    : [];

  return {
    items,
    page: 1,
    per_page: 10,
    total_count: items.length,
    total_pages: items.length > 0 ? 1 : 0,
  };
}

function publicEventList(state: MockState) {
  const event = state.event;

  return {
    items: event
      ? [
          {
            ...event,
            thumbnail: event.cover_image_id ? thumbnail() : null,
          },
        ]
      : [],
    next_cursor: null,
  };
}

function publicEventDetail(state: MockState, user: UserRecord | null) {
  const event = requireEvent(state);
  const host = [...state.users.values()].find(
    (candidate) => candidate.id === event.host_id,
  );
  const isRegistered =
    user &&
    state.registration?.user_id === user.id &&
    state.registration.status === 'registered';

  return {
    event: {
      ...event,
      thumbnail: event.cover_image_id ? thumbnail() : null,
      created_at: '2030-01-01T00:00:00.000Z',
      updated_at: '2030-01-01T00:00:00.000Z',
    },
    host: {
      id: event.host_id,
      display_name: host?.displayName ?? 'Host Organizer',
      avatar_object_key: null,
    },
    attendee_count: activeAttendeeCount(state),
    capacity_remaining:
      event.capacity === null
        ? null
        : event.capacity - activeAttendeeCount(state),
    current_user_registration_state: isRegistered ? 'registered' : null,
  };
}

function hostAttendees(state: MockState) {
  const attendee = state.registration
    ? [...state.users.values()].find(
        (user) => user.id === state.registration?.user_id,
      )
    : null;

  return {
    event_id: eventId,
    attendees:
      attendee && state.registration
        ? [
            {
              registration_id: state.registration.id,
              user_id: attendee.id,
              email: attendee.email,
              display_name: attendee.displayName,
              status: state.registration.status,
              registered_at: state.registration.registered_at,
              cancelled_at: state.registration.cancelled_at,
            },
          ]
        : [],
  };
}

function attendeeRegistrations(state: MockState) {
  const event = requireEvent(state);
  const host = [...state.users.values()].find(
    (candidate) => candidate.id === event.host_id,
  );
  const registration =
    state.registration?.status === 'registered'
      ? [
          {
            registration_id: state.registration.id,
            status: state.registration.status,
            registered_at: state.registration.registered_at,
            cancelled_at: state.registration.cancelled_at,
            event: {
              ...event,
              host_display_name: host?.displayName ?? 'Host Organizer',
            },
          },
        ]
      : [];

  return {
    items: registration,
    page: 1,
    per_page: 6,
    total_count: registration.length,
    total_pages: registration.length > 0 ? 1 : 0,
  };
}

function thumbnail() {
  return {
    object_key: `events/${eventId}/cover/thumb.png`,
    public_url: coverDataUrl,
    width: 480,
    height: 270,
    bytes: 456,
  };
}

function activeAttendeeCount(state: MockState) {
  return state.registration?.status === 'registered' ? 1 : 0;
}

function requireEvent(state: MockState) {
  if (!state.event) {
    throw new Error('Expected mock event to exist.');
  }

  return state.event;
}

function requireRegistration(state: MockState) {
  if (!state.registration) {
    throw new Error('Expected mock registration to exist.');
  }

  return state.registration;
}

function requireUser(state: MockState, email: string) {
  const user = state.users.get(email);

  if (!user) {
    throw new Error(`Expected mock user ${email} to exist.`);
  }

  return user;
}

function optionalUserFromAuthHeader(state: MockState, authorization?: string) {
  if (!authorization) {
    return null;
  }

  return userFromAuthHeader(state, authorization);
}

function userFromAuthHeader(state: MockState, authorization?: string) {
  const email = authorization?.replace(/^Bearer access:/, '');

  if (!email) {
    throw new Error('Expected bearer access token.');
  }

  return requireUser(state, email);
}

function displayNameFor(email: string) {
  return email === 'host@example.com' ? 'Host Organizer' : 'Second Attendee';
}
