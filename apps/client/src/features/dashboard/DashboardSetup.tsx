import { Badge, Button, Group, Progress, Stack, Text, ThemeIcon, Timeline } from '@mantine/core';
import { CheckCircle2, Circle, History } from 'lucide-react';
import type { ReactNode } from 'react';
import { Link } from 'react-router';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useBusinessActivity, useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { formatDate, formatGstNumber, formatRelative } from '@/lib/format';

interface ChecklistItem {
  label: string;
  description: string;
  done: boolean;
  to: string;
  action: string;
}

export function SetupChecklist({ hideWhenDone = false }: { hideWhenDone?: boolean }) {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  const hasAddress = !!business.address.line1 && !!business.address.city;
  const hasPaymentDetails = !!settings.data && (settings.data.bank_account_number !== '' || settings.data.payment_instructions !== '');
  const items: ChecklistItem[] = [
    {
      label: 'Add contact details and address',
      description: 'They appear on your invoices.',
      done: hasAddress && !!business.email,
      to: '/settings/business',
      action: 'Add details',
    },
    {
      label: 'Upload your logo',
      description: 'Make your documents look like you.',
      done: business.has_logo,
      to: '/settings/business',
      action: 'Upload logo',
    },
    {
      label: 'Add payment details',
      description: 'Tell customers how to pay you.',
      done: hasPaymentDetails,
      to: '/settings/invoicing',
      action: 'Add payment details',
    },
  ];
  const done = items.filter((i) => i.done).length;
  if (settings.isPending) return <ListSkeleton rows={4} />;
  if (hideWhenDone && done === items.length) return null;
  return (
    <SectionCard
      title="Finish setting up"
      description={done === items.length ? 'All done. Your business profile is complete.' : `${done} of ${items.length} done`}
    >
      <Progress value={(done / items.length) * 100} size="sm" mb="md" aria-label={`${done} of ${items.length} set-up steps complete`} />
      <Stack gap="sm">
        {items.map((item) => (
          <Group key={item.label} justify="space-between" wrap="wrap" gap="sm">
            <Group gap="sm" wrap="nowrap" style={{ minWidth: 0, flex: '1 1 220px' }}>
              <ThemeIcon variant="transparent" color={item.done ? 'teal' : 'gray'} size={22}>
                {item.done ? <CheckCircle2 size={20} /> : <Circle size={20} />}
              </ThemeIcon>
              <div style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} td={item.done ? 'line-through' : undefined} c={item.done ? 'dimmed' : undefined}>
                  {item.label}
                </Text>
                {!item.done && (
                  <Text size="xs" c="dimmed">
                    {item.description}
                  </Text>
                )}
              </div>
            </Group>
            {!item.done && (
              <Button component={Link} to={item.to} size="compact-sm" variant="light" ml="auto">
                {item.action}
              </Button>
            )}
          </Group>
        ))}
      </Stack>
    </SectionCard>
  );
}

export function BusinessSummary() {
  const business = useCurrentBusiness();
  const fy = business.current_financial_year;
  const rows: [string, ReactNode][] = [
    ['Financial year', `${fy.label} · ${formatDate(fy.start)} – ${formatDate(fy.end)}`],
    [
      'GST',
      business.gst_registered ? (
        <Badge color="teal">Registered{business.gst_number ? ` · ${formatGstNumber(business.gst_number)}` : ''}</Badge>
      ) : (
        <Badge color="gray">Not registered</Badge>
      ),
    ],
    ['Currency', business.currency],
    ['Reporting basis', business.reporting_basis === 'cash' ? 'Cash' : 'Accrual'],
  ];
  return (
    <SectionCard
      title="Your business"
      actions={
        <Button component={Link} to="/settings/business" variant="subtle" size="compact-sm">
          Edit
        </Button>
      }
    >
      <Stack gap="sm">
        {rows.map(([label, value]) => (
          <Group key={label} justify="space-between" gap="sm">
            <Text size="sm" c="dimmed">
              {label}
            </Text>
            {typeof value === 'string' ? (
              <Text size="sm" fw={500}>
                {value}
              </Text>
            ) : (
              value
            )}
          </Group>
        ))}
      </Stack>
    </SectionCard>
  );
}

export function RecentActivity() {
  const business = useCurrentBusiness();
  const activity = useBusinessActivity(business.id, 1);
  const items = activity.data?.items.slice(0, 6) ?? [];
  return (
    <SectionCard
      title="Recent activity"
      actions={
        <Button component={Link} to="/settings/activity" variant="subtle" size="compact-sm">
          View all
        </Button>
      }
    >
      {activity.isPending ? (
        <ListSkeleton rows={3} height={36} />
      ) : activity.isError ? (
        <ErrorState error={activity.error} onRetry={() => void activity.refetch()} />
      ) : items.length === 0 ? (
        <EmptyBlock compact icon={History} title="Nothing yet" description="Changes you make will be listed here." />
      ) : (
        <Timeline bulletSize={10} lineWidth={2}>
          {items.map((entry) => (
            <Timeline.Item key={entry.id}>
              <Text size="sm">{entry.summary}</Text>
              <Text size="xs" c="dimmed">
                {entry.actor_name ?? 'System'} · {formatRelative(entry.created_at)}
              </Text>
            </Timeline.Item>
          ))}
        </Timeline>
      )}
    </SectionCard>
  );
}
