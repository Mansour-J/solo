import { Button, Group, Select, SimpleGrid, Stack, Tabs } from '@mantine/core';
import type { Business } from '@solo/api-client';
import { History, Inbox, PiggyBank } from 'lucide-react';
import { useState } from 'react';
import { useSearchParams } from 'react-router';
import { BRAND } from '@/branding/brand';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { periodRange, type Period } from '@/lib/periods';
import { useAllocationBalances, useDestinations, type AllocationStatus } from './api';
import { AllocationList } from './AllocationList';
import { BalancesPanel } from './BalancesPanel';
import { DestinationsPanel } from './DestinationsPanel';
import { STATUS_LABELS } from './format';
import { ReviewModal } from './ReviewModal';
import { RulesPanel } from './RulesPanel';
import { BackfillButton, SpendingPanel } from './SpendingPanel';

type Tab = 'overview' | 'history' | 'spending' | 'rules' | 'destinations';
const TABS: Tab[] = ['overview', 'history', 'spending', 'rules', 'destinations'];

const DESCRIPTION = `Plan where the money you receive should go. ${BRAND.name} records your plan; it never moves money.`;

function HistoryTab({ business, onOpen }: { business: Business; onOpen: (id: string) => void }) {
  const destinations = useDestinations();
  const [status, setStatus] = useState<AllocationStatus | null>(null);
  const [destinationId, setDestinationId] = useState<string | null>(null);
  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const params = {
    status: status ?? undefined,
    destination_id: destinationId ?? undefined,
    from: period.from,
    to: period.to,
  };
  return (
    <Stack gap="md">
      <Group gap="xs" wrap="wrap" align="flex-start">
        <PeriodPicker value={period} onChange={setPeriod} today={business.today} fyStart={business.current_financial_year.start} />
        <Select
          aria-label="Status"
          placeholder="Any status"
          clearable
          w={{ base: '100%', xs: 180 }}
          data={Object.entries(STATUS_LABELS).map(([value, s]) => ({ value, label: s.label }))}
          value={status}
          onChange={(value) => setStatus(value as AllocationStatus | null)}
        />
        <Select
          aria-label="Destination"
          placeholder="All destinations"
          clearable
          searchable
          w={{ base: '100%', xs: 220 }}
          data={(destinations.data ?? []).map((d) => ({ value: d.id, label: d.archived ? `${d.name} (archived)` : d.name }))}
          value={destinationId}
          onChange={setDestinationId}
        />
      </Group>
      <AllocationList
        key={JSON.stringify(params)}
        params={params}
        onOpen={(a) => onOpen(a.id)}
        empty={
          <EmptyBlock
            icon={History}
            title="No allocations in this period"
            description="Allocations of payments and income you receive appear here."
          />
        }
      />
    </Stack>
  );
}

function Overview({ onOpen, onStart }: { onOpen: (id: string) => void; onStart: () => void }) {
  const balances = useAllocationBalances();
  if (balances.isPending) return <ListSkeleton rows={5} />;
  if (balances.isError) return <ErrorState error={balances.error} onRetry={() => void balances.refetch()} />;
  if (!balances.data.has_rules && balances.data.total === 0 && balances.data.pending_count === 0) {
    return (
      <EmptyBlock
        icon={PiggyBank}
        title="Plan where your money goes"
        description={`Add rules to set aside part of each payment for income tax, GST, savings and more. When you record money received, ${BRAND.name} proposes the split and you confirm it. It’s a plan: ${BRAND.name} never moves money.`}
        action={<Button onClick={onStart}>Add your first rule</Button>}
      />
    );
  }
  return (
    <SimpleGrid cols={{ base: 1, lg: 2 }} spacing="lg">
      <BalancesPanel balances={balances.data} loading={false} />
      <SectionCard
        title="To review"
        actions={balances.data.has_rules ? <BackfillButton /> : undefined}
        description={
          balances.data.pending_count > 0
            ? `${balances.data.pending_count} ${balances.data.pending_count === 1 ? 'payment' : 'payments'} waiting for you to confirm the split.`
            : undefined
        }
      >
        <AllocationList
          compact
          params={{ status: 'pending' }}
          onOpen={(a) => onOpen(a.id)}
          empty={
            <EmptyBlock
              compact
              icon={Inbox}
              title="Nothing to review"
              description="When you record a payment or income, its proposed split appears here."
            />
          }
        />
      </SectionCard>
    </SimpleGrid>
  );
}

/** Balances, allocations to review, history, rules and destinations. `?review=<id>` opens an allocation. */
export function AllocationsPage() {
  const business = useCurrentBusiness();
  const [searchParams, setSearchParams] = useSearchParams();
  const requested = searchParams.get('tab') as Tab | null;
  const tab: Tab = requested && TABS.includes(requested) ? requested : 'overview';
  const reviewId = searchParams.get('review');
  const creatingRule = tab === 'rules' && searchParams.get('new') === '1';
  const updateParams = (change: (next: URLSearchParams) => void) =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        change(next);
        return next;
      },
      { replace: true },
    );
  const open = (id: string) => updateParams((p) => p.set('review', id));

  return (
    <Page title="Allocations" description={DESCRIPTION}>
      <Tabs
        value={tab}
        onChange={(value) =>
          updateParams((p) => {
            p.delete('new');
            if (!value || value === 'overview') p.delete('tab');
            else p.set('tab', value);
          })
        }
      >
        <Tabs.List mb="md">
          <Tabs.Tab value="overview">Overview</Tabs.Tab>
          <Tabs.Tab value="history">History</Tabs.Tab>
          <Tabs.Tab value="spending">Spending</Tabs.Tab>
          <Tabs.Tab value="rules">Rules</Tabs.Tab>
          <Tabs.Tab value="destinations">Destinations</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="overview">
          {tab === 'overview' && (
            <Overview
              onOpen={open}
              onStart={() =>
                updateParams((p) => {
                  p.set('tab', 'rules');
                  p.set('new', '1');
                })
              }
            />
          )}
        </Tabs.Panel>
        <Tabs.Panel value="history">{tab === 'history' && <HistoryTab key={business.id} business={business} onOpen={open} />}</Tabs.Panel>
        <Tabs.Panel value="spending">{tab === 'spending' && <SpendingPanel />}</Tabs.Panel>
        <Tabs.Panel value="rules">
          {tab === 'rules' && (
            <RulesPanel
              creating={creatingRule}
              onCreate={() => updateParams((p) => p.set('new', '1'))}
              onCloseCreate={() => updateParams((p) => p.delete('new'))}
            />
          )}
        </Tabs.Panel>
        <Tabs.Panel value="destinations">{tab === 'destinations' && <DestinationsPanel />}</Tabs.Panel>
      </Tabs>
      <ReviewModal allocationId={reviewId} onClose={() => updateParams((p) => p.delete('review'))} />
    </Page>
  );
}
