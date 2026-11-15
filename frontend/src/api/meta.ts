import { z } from 'zod'
import { getJson } from './client'
import type { AppInfo } from './generated/AppInfo'

const appInfoSchema = z.object({ name: z.string(), version: z.string() }) satisfies z.ZodType<AppInfo>

export function fetchAppInfo(signal?: AbortSignal): Promise<AppInfo> {
  return getJson('/api/version', appInfoSchema, signal)
}
