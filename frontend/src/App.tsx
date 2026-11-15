import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createBrowserRouter } from 'react-router'
import { createQueryClient } from './app/queryClient'
import { routes } from './app/routes'

const queryClient = createQueryClient()
const router = createBrowserRouter(routes)

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="min-h-dvh bg-brand-50 text-brand-900">
        <RouterProvider router={router} />
      </div>
    </QueryClientProvider>
  )
}
