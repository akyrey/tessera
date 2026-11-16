/** What the card form will do with one side's photo when saved. */
export type PhotoChange =
  | { readonly type: 'keep' }
  | { readonly type: 'replace'; readonly photo: Blob; readonly preview: string }
  | { readonly type: 'remove' }

export const KEEP_PHOTO: PhotoChange = { type: 'keep' }
