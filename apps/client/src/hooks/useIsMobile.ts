import { useMediaQuery } from '@mantine/hooks';

/** Below Mantine's `sm` breakpoint (48em ≈ 768px) the app uses its touch layout. */
export function useIsMobile(): boolean {
  return useMediaQuery('(max-width: 47.99em)', false, { getInitialValueInEffect: false }) ?? false;
}
