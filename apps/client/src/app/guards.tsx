import { Center, Loader } from '@mantine/core';
import { Navigate, Outlet } from 'react-router';
import { ErrorState } from '@/components/States';
import { resolveCurrentBusiness, useBusiness, useBusinesses } from '@/features/businesses/api';
import { useSessionStore } from '@/stores/session';

export function PageLoader() {
  return (
    <Center h="60vh" aria-busy="true">
      <Loader size="sm" type="dots" aria-label="Loading" />
    </Center>
  );
}

/** Routes that work inside a business. Sends users without one to onboarding. */
export function RequireBusiness() {
  const selectedId = useSessionStore((s) => s.currentBusinessId);
  const businesses = useBusinesses();
  const current = resolveCurrentBusiness(businesses.data, selectedId);
  const business = useBusiness(current?.id);

  if (businesses.isPending) return <PageLoader />;
  if (businesses.isError) return <ErrorState error={businesses.error} onRetry={() => void businesses.refetch()} />;
  if (!current) return <Navigate to="/onboarding" replace />;
  if (business.isError) return <ErrorState error={business.error} onRetry={() => void business.refetch()} />;
  if (!business.data) return <PageLoader />;
  return <Outlet />;
}
