import { createBrowserRouter } from 'react-router-dom';

import { ProtectedRoute } from '@/components/protected-route';
import { RootLayout } from '@/components/root-layout';
import { AttendeePage } from '@/pages/attendee-page';
import { AuthPage } from '@/pages/auth-page';
import { ForgotPasswordPage } from '@/pages/forgot-password-page';
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
