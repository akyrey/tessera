import { Navigate, type RouteObject } from 'react-router'
import { JoinPage } from '../features/auth/JoinPage'
import { LoginPage } from '../features/auth/LoginPage'
import { RequireAuth } from '../features/auth/RequireAuth'
import { EditCardPage, NewCardPage } from '../features/cards/CardFormPage'
import { CardListPage } from '../features/cards/CardListPage'
import { CardViewPage } from '../features/cards/CardViewPage'
import { SettingsPage } from '../features/settings/SettingsPage'

export const routes: RouteObject[] = [
  { path: '/login', element: <LoginPage /> },
  { path: '/join', element: <JoinPage /> },
  {
    element: <RequireAuth />,
    children: [
      { index: true, element: <CardListPage /> },
      { path: 'cards/new', element: <NewCardPage /> },
      { path: 'cards/:id', element: <CardViewPage /> },
      { path: 'cards/:id/edit', element: <EditCardPage /> },
      { path: 'settings', element: <SettingsPage /> },
    ],
  },
  { path: '*', element: <Navigate to="/" replace /> },
]
