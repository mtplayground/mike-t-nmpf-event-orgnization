import { createBrowserRouter } from 'react-router-dom';

import { ProtectedRoute } from '@/components/protected-route';
import { RootLayout } from '@/components/root-layout';
import { AttendeePage } from '@/pages/attendee-page';
import { AuthPage } from '@/pages/auth-page';
import { EventDetailPage } from '@/pages/event-detail-page';
import { ForgotPasswordPage } from '@/pages/forgot-password-page';
import { HostEventAttendeesPage } from '@/pages/host-event-attendees-page';
import { HostEventFormPage } from '@/pages/host-event-form-page';
import { HomePage } from '@/pages/home-page';
import { HostPage } from '@/pages/host-page';
import { LoginPage } from '@/pages/login-page';
import { NotFoundPage } from '@/pages/not-found-page';
import { ProfilePage } from '@/pages/profile-page';
import { RegisterPage } from '@/pages/register-page';
import { ResetPasswordPage } from '@/pages/reset-password-page';
import { VerifyEmailPage } from '@/pages/verify-email-page';

export const router = createBrowserRouter([
  {
    path: '/',
    element: <RootLayout />,
    children: [
      { index: true, element: <HomePage /> },
      { path: 'auth', element: <AuthPage /> },
      { path: 'auth/login', element: <LoginPage /> },
      { path: 'auth/register', element: <RegisterPage /> },
      { path: 'auth/verify-email', element: <VerifyEmailPage /> },
      { path: 'auth/forgot-password', element: <ForgotPasswordPage /> },
      { path: 'auth/reset-password', element: <ResetPasswordPage /> },
      { path: 'events/:slug', element: <EventDetailPage /> },
      {
        path: 'profile',
        element: (
          <ProtectedRoute>
            <ProfilePage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host',
        element: (
          <ProtectedRoute>
            <HostPage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/new',
        element: (
          <ProtectedRoute>
            <HostEventFormPage mode="create" />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/:eventId/edit',
        element: (
          <ProtectedRoute>
            <HostEventFormPage mode="edit" />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/:eventId/attendees',
        element: (
          <ProtectedRoute>
            <HostEventAttendeesPage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'attendee',
        element: (
          <ProtectedRoute>
            <AttendeePage />
          </ProtectedRoute>
        ),
      },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]);
