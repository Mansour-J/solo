import { Button, Group, SegmentedControl, SimpleGrid, Stack, Text } from '@mantine/core';
import { AlertTriangle, BookOpen, Car, FileText, Plus, Receipt, Scale, TrendingDown, TrendingUp, UserPlus, Wallet } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { ErrorState, SectionCard } from '@/components/States';
import { useCurrentUser } from '@/features/account/api';
import { AllocationsCard } from '@/features/allocations/AllocationsCard';
import { DashboardNotices } from '@/features/insights/DashboardNotices';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate } from '@/lib/format';
import { periodRange, type Period } from '@/lib/periods';
import { useDashboard, type Basis } from './api';
import { CategoryBreakdown, GstCard, KpiCard, MileageCard, MonthlyChart, RecentInvoices } from './DashboardPanels';
import { KeyDates } from './KeyDates';
import { BusinessSummary, RecentActivity, SetupChecklist } from './DashboardSetup';

function QuickActions() {
  return (
    <Group gap="xs" wrap="wrap">
      <Button component={Link} to="/invoices/new" leftSection={<FileText size={15} />}>
        New invoice
      </Button>
      <Button component={Link} to="/expenses?new=1" variant="default" leftSection={<Receipt size={15} />}>
        Add expense
      </Button>
      <Button component={Link} to="/income?new=1" variant="default" leftSection={<Plus size={15} />}>
        Record income
      </Button>
      <Button component={Link} to="/mileage?new=1" variant="default" leftSection={<Car size={15} />}>
        Record mileage
      </Button>
      <Button component={Link} to="/customers?new=1" variant="default" leftSection={<UserPlus size={15} />}>
        Add customer
      </Button>
    </Group>
  );
}

function GettingStarted() {
  const steps = [
    {
      title: 'Add a customer',
      text: 'Who you work for: their contact details and payment terms.',
      to: '/customers?new=1',
      action: 'Add customer',
    },
    {
      title: 'Send your first invoice',
      text: 'Draft it, issue it, and email the PDF straight from here.',
      to: '/invoices/new',
      action: 'New invoice',
    },
    {
      title: 'Record an expense',
      text: 'Snap the receipt and attach it, so it’s there at tax time.',
      to: '/expenses?new=1',
      action: 'Add expense',
    },
  ];
  return (
    <SectionCard
      title="Get started"
      description="Your figures appear here as soon as you record income or expenses."
      actions={
        <Button component={Link} to="/guide" variant="subtle" size="xs" leftSection={<BookOpen size={14} />}>
          Step-by-step guide
        </Button>
      }
    >
      <SimpleGrid cols={{ base: 1, md: 3 }}>
        {steps.map((s, i) => (
          <Stack key={s.title} gap={6}>
            <Text size="xs" c="dimmed" fw={600}>
              Step {i + 1}
            </Text>
            <Text fw={600}>{s.title}</Text>
            <Text size="sm" c="dimmed">
              {s.text}
            </Text>
            <Button component={Link} to={s.to} variant="light" size="xs" w="fit-content">
              {s.action}
            </Button>
          </Stack>
        ))}
      </SimpleGrid>
    </SectionCard>
  );
}

export function DashboardPage() {
  const user = useCurrentUser();
  const business = useCurrentBusiness();
  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const [basis, setBasis] = useState<Basis>(business.reporting_basis);
  const dashboard = useDashboard(business.id, period.from, period.to, basis);
  const data = dashboard.data;
  const loading = dashboard.isPending;
  const currency = business.currency;
  const empty = data?.is_empty ?? false;

  return (
    <Page
      title={user.first_name ? `Kia ora, ${user.first_name}` : 'Kia ora'}
      description={`${business.name} · ${formatDate(period.from)} – ${formatDate(period.to)}`}
      actions={
        <Group gap="xs" wrap="wrap">
          <PeriodPicker value={period} onChange={setPeriod} today={business.today} fyStart={business.current_financial_year.start} />
          <SegmentedControl
            aria-label="Reporting basis"
            value={basis}
            onChange={(v) => setBasis(v as Basis)}
            data={[
              { value: 'accrual', label: 'Accrual' },
              { value: 'cash', label: 'Cash' },
            ]}
          />
        </Group>
      }
    >
      <QuickActions />
      <DashboardNotices />
      {dashboard.isError ? (
        <ErrorState error={dashboard.error} onRetry={() => void dashboard.refetch()} />
      ) : empty ? (
        <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
          <Stack gap="lg">
            <GettingStarted />
            <SetupChecklist />
          </Stack>
          <Stack gap="lg">
            <BusinessSummary />
            <RecentActivity />
          </Stack>
        </SimpleGrid>
      ) : (
        <>
          <SimpleGrid cols={{ base: 1, xs: 2, lg: 5 }} spacing="md">
            <KpiCard
              label={basis === 'cash' ? 'Income received' : 'Income earned'}
              icon={TrendingUp}
              loading={loading}
              value={<MoneyText value={data?.totals.income ?? 0} currency={currency} inherit />}
              current={data?.totals.income}
              previous={data?.previous_totals.income}
            />
            <KpiCard
              label="Expenses"
              icon={TrendingDown}
              loading={loading}
              inverse
              value={<MoneyText value={data?.totals.expenses ?? 0} currency={currency} inherit />}
              current={data?.totals.expenses}
              previous={data?.previous_totals.expenses}
            />
            <KpiCard
              label="Net profit"
              icon={Scale}
              loading={loading}
              value={<MoneyText value={data?.totals.net_profit ?? 0} currency={currency} inherit />}
              current={data?.totals.net_profit}
              previous={data?.previous_totals.net_profit}
            />
            <KpiCard
              label="Outstanding"
              icon={Wallet}
              loading={loading}
              to="/invoices?tab=unpaid"
              value={<MoneyText value={data?.receivables.outstanding_amount ?? 0} currency={currency} inherit />}
              hint={`${data?.receivables.outstanding_count ?? 0} unpaid · ${data?.receivables.draft_count ?? 0} drafts`}
            />
            <KpiCard
              label="Overdue"
              icon={AlertTriangle}
              loading={loading}
              to="/invoices?tab=overdue"
              value={
                <MoneyText
                  value={data?.receivables.overdue_amount ?? 0}
                  currency={currency}
                  inherit
                  c={(data?.receivables.overdue_amount ?? 0) > 0 ? 'red' : undefined}
                />
              }
              hint={`${data?.receivables.overdue_count ?? 0} invoices`}
            />
          </SimpleGrid>
          <SimpleGrid cols={{ base: 1, lg: 3 }} spacing="lg">
            <div className="span-2-lg" style={{ minWidth: 0 }}>
              <MonthlyChart data={data} loading={loading} />
            </div>
            <CategoryBreakdown data={data} loading={loading} />
          </SimpleGrid>
          <KeyDates />
          <SimpleGrid cols={{ base: 1, md: 2, lg: 3 }} spacing="lg">
            <RecentInvoices data={data} />
            <Stack gap="lg">
              <GstCard data={data} />
              <MileageCard data={data} />
              <AllocationsCard />
              <SetupChecklist hideWhenDone />
            </Stack>
            <RecentActivity />
          </SimpleGrid>
        </>
      )}
    </Page>
  );
}
