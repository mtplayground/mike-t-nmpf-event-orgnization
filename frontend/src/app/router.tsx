import { createBrowserRouter } from 'react-router-dom';

import { ProtectedRoute } from '@/components/protected-route';
import { RootLayout } from '@/components/root-layout';
import { RouteErrorPage } from '@/components/route-error-page';
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
    errorElement: <RouteErrorPage />,
    children: [
      { index: true, element: <HomePage />, errorElement: <RouteErrorPage /> },
      { path: 'auth', element: <AuthPage />, errorElement: <RouteErrorPage /> },
      {
        path: 'auth/login',
        element: <LoginPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'auth/register',
        element: <RegisterPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'auth/verify-email',
        element: <VerifyEmailPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'auth/forgot-password',
        element: <ForgotPasswordPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'auth/reset-password',
        element: <ResetPasswordPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'events/:slug',
        element: <EventDetailPage />,
        errorElement: <RouteErrorPage />,
      },
      {
        path: 'profile',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <ProfilePage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <HostPage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/new',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <HostEventFormPage mode="create" />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/:eventId/edit',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <HostEventFormPage mode="edit" />
          </ProtectedRoute>
        ),
      },
      {
        path: 'host/events/:eventId/attendees',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <HostEventAttendeesPage />
          </ProtectedRoute>
        ),
      },
      {
        path: 'attendee',
        errorElement: <RouteErrorPage />,
        element: (
          <ProtectedRoute>
            <AttendeePage />
          </ProtectedRoute>
        ),
      },
      {
        path: '*',
        element: <NotFoundPage />,
        errorElement: <RouteErrorPage />,
      },
    ],
  },
]);
