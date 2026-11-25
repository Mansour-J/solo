import { Box, Button, Group, Menu, NavLink, Paper, SegmentedControl, Select, SimpleGrid, Stack, Text } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import type { Business } from '@solo/api-client';
import { ChevronDown, Download, Files, FileSpreadsheet, FileText, Users } from 'lucide-react';
import { useState } from 'react';
import { useSearchParams } from 'react-router';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { CustomerSelect } from '@/features/customers/CustomerSelect';
import { useExpenseCategories } from '@/features/expenses/api';
import { useVehicles } from '@/features/mileage/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { periodRange, type Period } from '@/lib/periods';
import { errorMessage } from '@/services/api/errors';
import {
  downloadAccountantPack,
  downloadXero,
  downloadAllReports,
  downloadReport,
  useReport,
  type ExportFormat,
  type ReportKind,
  type ReportParams,
} from './api';
import { REPORTS, reportInfo, type ReportInfo } from './catalogue';
import { ReportView } from './ReportView';

const STATUS_OPTIONS = [
  { value: 'pending', label: 'To review' },
  { value: 'confirmed', label: 'Allocated' },
  { value: 'skipped', label: 'Skipped' },
  { value: 'reversed', label: 'Reversed' },
  { value: 'cancelled', label: 'Cancelled' },
];

function CategoryFilter({ value, onChange }: { value: string | null; onChange: (v: string | null) => void }) {
  const categories = useExpenseCategories();
  return (
    <Select
      aria-label="Category"
      placeholder="All categories"
      clearable
      searchable
      w={{ base: '100%', xs: 220 }}
      data={(categories.data ?? []).map((c) => ({ value: c.id, label: c.archived ? `${c.name} (archived)` : c.name }))}
      value={value}
      onChange={onChange}
    />
  );
}

function VehicleFilter({ value, onChange }: { value: string | null; onChange: (v: string | null) => void }) {
  const vehicles = useVehicles();
  if (!vehicles.data || vehicles.data.length < 2) return null;
  return (
    <Select
      aria-label="Vehicle"
      placeholder="All vehicles"
      clearable
      w={{ base: '100%', xs: 200 }}
      data={vehicles.data.map((v) => ({ value: v.id, label: v.archived ? `${v.name} (archived)` : v.name }))}
      value={value}
      onChange={onChange}
    />
  );
}

function ReportPanel({ business, info }: { business: Business; info: ReportInfo }) {
  const [searchParams, setSearchParams] = useSearchParams();
  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const [basis, setBasis] = useState<'cash' | 'accrual' | null>(null);
  const [exporting, setExporting] = useState<string | null>(null);
  const customer = searchParams.get('customer');
  const category = searchParams.get('category');
  const vehicle = searchParams.get('vehicle');
  const status = searchParams.get('status') as ReportParams['status'] | null;
  const setParam = (key: string, value: string | null) =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        if (value) next.set(key, value);
        else next.delete(key);
        return next;
      },
      { replace: true },
    );
  const params: ReportParams = {
    ...(info.period ? { from: period.from, to: period.to } : {}),
    ...(info.basis && basis ? { basis } : {}),
    ...(info.customer && customer ? { customer_id: customer } : {}),
    ...(info.category && category ? { category_id: category } : {}),
    ...(info.vehicle && vehicle ? { vehicle_id: vehicle } : {}),
    ...(info.status && status ? { status } : {}),
  };
  const waiting = info.customer === 'required' && !customer;
  const report = useReport(info.kind, params, !waiting);

  const exportAs = async (format: ExportFormat) => {
    if (!report.data) return;
    setExporting(format);
    try {
      await downloadReport(business.id, info.kind, format, params, report.data);
    } catch (error) {
      notifications.show({ color: 'red', title: 'Couldn’t export the report', message: errorMessage(error) });
    } finally {
      setExporting(null);
    }
  };

  /** Every report for this period and basis (the other filters apply to one report only). */
  const exportAll = async (format: ExportFormat) => {
    setExporting(`all-${format}`);
    try {
      await downloadAllReports(business.id, format, { from: period.from, to: period.to, ...(basis ? { basis } : {}) }, business.name);
    } catch (error) {
      notifications.show({ color: 'red', title: 'Couldn’t export the reports', message: errorMessage(error) });
    } finally {
      setExporting(null);
    }
  };
  // The last financial year that has ended: the day before this one starts.
  const lastYearEnd = new Date(Date.parse(`${business.current_financial_year.start}T00:00:00Z`) - 86_400_000).toISOString().slice(0, 10);
  const exportPack = async () => {
    setExporting('pack');
    try {
      await downloadAccountantPack(business.id, lastYearEnd, basis ?? undefined, business.name);
    } catch (error) {
      notifications.show({ color: 'red', title: 'Couldn’t make the accountant pack', message: errorMessage(error) });
    } finally {
      setExporting(null);
    }
  };
  const exportXero = async (kind: 'sales' | 'bills') => {
    setExporting(kind);
    try {
      await downloadXero(business.id, kind, period.from, period.to);
    } catch (error) {
      notifications.show({ color: 'red', title: 'Couldn’t export for Xero', message: errorMessage(error) });
    } finally {
      setExporting(null);
    }
  };
  const ready = !!report.data && !waiting;

  let content;
  if (waiting) {
    content = (
      <EmptyBlock icon={Users} title="Choose a customer" description="A statement shows one customer’s invoices, payments and balance." />
    );
  } else if (report.isPending) content = <ListSkeleton rows={6} height={44} />;
  else if (report.isError) content = <ErrorState error={report.error} onRetry={() => void report.refetch()} />;
  else {
    content = (
      <Box style={{ opacity: report.isPlaceholderData ? 0.6 : 1, transition: 'opacity 120ms' }}>
        <ReportView report={report.data} />
      </Box>
    );
  }

  return (
    <Stack gap="md" style={{ minWidth: 0 }}>
      <Group justify="space-between" align="flex-start" gap="sm" wrap="wrap">
        <Box style={{ minWidth: 0, flex: '1 1 260px' }}>
          <Text fw={600} size="lg">
            {info.label}
          </Text>
          <Text size="sm" c="dimmed">
            {info.description}
          </Text>
        </Box>
        <Menu position="bottom-end" withinPortal>
          <Menu.Target>
            <Button
              variant="default"
              leftSection={<Download size={16} />}
              rightSection={<ChevronDown size={14} />}
              loading={exporting !== null}
            >
              Export
            </Button>
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Label>{info.label}</Menu.Label>
            <Menu.Item leftSection={<FileText size={15} />} disabled={!ready} onClick={() => void exportAs('pdf')}>
              PDF
            </Menu.Item>
            <Menu.Item leftSection={<FileSpreadsheet size={15} />} disabled={!ready} onClick={() => void exportAs('xlsx')}>
              Excel (.xlsx)
            </Menu.Item>
            <Menu.Item leftSection={<FileSpreadsheet size={15} />} disabled={!ready} onClick={() => void exportAs('csv')}>
              CSV
            </Menu.Item>
            <Menu.Divider />
            <Menu.Label>Export all reports</Menu.Label>
            <Menu.Item leftSection={<Files size={15} />} onClick={() => void exportAll('pdf')}>
              All reports as PDF
            </Menu.Item>
            <Menu.Item leftSection={<Files size={15} />} onClick={() => void exportAll('xlsx')}>
              All reports as Excel (.xlsx)
            </Menu.Item>
            <Menu.Item leftSection={<Files size={15} />} onClick={() => void exportAll('csv')}>
              All reports as CSV
            </Menu.Item>
            <Menu.Divider />
            <Menu.Label>For your accountant</Menu.Label>
            <Menu.Item leftSection={<Files size={15} />} onClick={() => void exportPack()}>
              Accountant pack for last year (.zip)
            </Menu.Item>
            <Menu.Divider />
            <Menu.Label>For Xero (this period)</Menu.Label>
            <Menu.Item leftSection={<FileSpreadsheet size={15} />} onClick={() => void exportXero('sales')}>
              Xero sales invoices (CSV)
            </Menu.Item>
            <Menu.Item leftSection={<FileSpreadsheet size={15} />} onClick={() => void exportXero('bills')}>
              Xero bills from expenses (CSV)
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      </Group>
      <Group gap="xs" wrap="wrap" align="flex-start">
        {info.period && (
          <PeriodPicker value={period} onChange={setPeriod} today={business.today} fyStart={business.current_financial_year.start} />
        )}
        {info.basis && (
          <SegmentedControl
            aria-label="Basis"
            value={basis ?? business.reporting_basis}
            onChange={(value) => setBasis(value as 'cash' | 'accrual')}
            data={[
              { value: 'accrual', label: 'Accrual' },
              { value: 'cash', label: 'Cash' },
            ]}
          />
        )}
        {info.customer && (
          <CustomerSelect
            aria-label="Customer"
            placeholder={info.customer === 'required' ? 'Choose a customer' : 'All customers'}
            clearable={info.customer === 'optional'}
            w={{ base: '100%', xs: 240 }}
            value={customer}
            onChange={(id) => setParam('customer', id)}
          />
        )}
        {info.category && <CategoryFilter value={category} onChange={(id) => setParam('category', id)} />}
        {info.vehicle && <VehicleFilter value={vehicle} onChange={(id) => setParam('vehicle', id)} />}
        {info.status && (
          <Select
            aria-label="Status"
            placeholder="Any status"
            clearable
            w={{ base: '100%', xs: 180 }}
            data={STATUS_OPTIONS}
            value={status}
            onChange={(value) => setParam('status', value)}
          />
        )}
      </Group>
      {content}
    </Stack>
  );
}

/** Every report, with shared filters and PDF, Excel and CSV exports. `?report=<kind>&customer=<id>` deep-links. */
export function ReportsPage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const [searchParams, setSearchParams] = useSearchParams();
  const info = reportInfo(searchParams.get('report'));
  const choose = (kind: ReportKind) =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams();
        next.set('report', kind);
        // Keep the customer when moving between reports that use one.
        const customer = previous.get('customer');
        if (customer && reportInfo(kind).customer) next.set('customer', customer);
        return next;
      },
      { replace: true },
    );

  return (
    <Page title="Reports" description="Figures from your records, for the period and basis you choose.">
      {isMobile ? (
        <Stack gap="md">
          <Select
            aria-label="Report"
            allowDeselect={false}
            data={REPORTS.map((r) => ({ value: r.kind, label: r.label }))}
            value={info.kind}
            onChange={(value) => value && choose(value as ReportKind)}
          />
          <ReportPanel key={`${business.id}-${info.kind}`} business={business} info={info} />
        </Stack>
      ) : (
        <SimpleGrid cols={2} spacing="lg" style={{ gridTemplateColumns: '240px minmax(0, 1fr)' }}>
          <Paper withBorder radius="lg" p="xs" style={{ alignSelf: 'start' }}>
            <nav aria-label="Reports">
              {REPORTS.map((r) => (
                <NavLink
                  key={r.kind}
                  label={r.label}
                  active={r.kind === info.kind}
                  onClick={() => choose(r.kind)}
                  aria-current={r.kind === info.kind ? 'page' : undefined}
                  style={{ borderRadius: 8 }}
                />
              ))}
            </nav>
          </Paper>
          <ReportPanel key={`${business.id}-${info.kind}`} business={business} info={info} />
        </SimpleGrid>
      )}
    </Page>
  );
}
