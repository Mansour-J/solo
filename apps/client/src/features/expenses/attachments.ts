import { MIME_TYPES } from '@mantine/dropzone';
import { formatNumber } from '@/lib/format';

/** Receipt files the API accepts (it checks the bytes, not just the type). */
export const RECEIPT_TYPES = [MIME_TYPES.pdf, MIME_TYPES.jpeg, MIME_TYPES.png, MIME_TYPES.webp];
export const MAX_RECEIPT_BYTES = 10 * 1024 * 1024;
export const MAX_RECEIPTS = 10;

export function isImage(contentType: string): boolean {
  return contentType.startsWith('image/');
}

/** File size for people, e.g. "820 KB", "2.4 MB". */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${formatNumber(bytes / 1024)} KB`;
  return `${formatNumber(bytes / (1024 * 1024), 1)} MB`;
}

/** Why dropped files were refused, from react-dropzone's error codes. */
export function rejectionMessage(codes: string[], remaining: number): string {
  if (codes.includes('too-many-files')) {
    return remaining > 0
      ? `You can add ${remaining} more file${remaining === 1 ? '' : 's'} to this expense.`
      : 'An expense can have up to 10 attachments.';
  }
  if (codes.includes('file-too-large')) return 'Receipts can be up to 10 MB.';
  return 'Use a PDF, JPEG, PNG or WebP file of up to 10 MB.';
}
