import { Badge, Group, Paper, SimpleGrid, Skeleton, Stack, Text, UnstyledButton } from '@mantine/core';
import { Link } from 'react-router';
import { MoneyText } from '@/components/Money';
import { ErrorState, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate } from '@/lib/format';
import { daysLeftLabel, useKeyDates, type KeyDate } from './api';

const URGENCY = {
  overdue: { color: 'red', label: 'Overdue' },
  due_soon: { color: 'orange', label: 'Due soon' },
  upcoming: { color: 'gray', label: 'Upcoming' },
} as const;

function KeyDateBox({ item, currency }: { item: KeyDate; currency: string }) {
  const urgency = URGENCY[item.urgency];
  return (
    <UnstyledButton component={Link} to={item.link} h="100%" aria-label={`${item.title}, ${daysLeftLabel(item.days_left)}: open`}>
      <Paper
        withBorder
        radius="md"
        p="sm"
        h="100%"
        style={item.urgency === 'upcoming' ? undefined : { borderColor: `var(--mantine-color-${urgency.color}-filled)` }}
      >
        <Stack gap={4}>
          <Group justify="space-between" gap={6} wrap="nowrap">
            <Text size="sm" fw={600} className="clamp-1">
              {item.title}
            </Text>
            {item.urgency !== 'upcoming' && (
              <Badge size="xs" variant="light" color={urgency.color}>
                {urgency.label}
              </Badge>
            )}
          </Group>
          <Text fz={20} fw={650} lh={1.2} className="tabular" c={item.urgency === 'overdue' ? 'red' : undefined}>
            {daysLeftLabel(item.days_left)}
          </Text>
          <Text size="xs" c="dimmed">
            {formatDate(item.date)}
            {item.amount ? (
              <>
                {' · '}
                <MoneyText value={item.amount} currency={currency} size="xs" inherit />
              </>
            ) : null}
          </Text>
          <Text size="xs" c="dimmed" className="clamp-1">
            {item.detail}
          </Text>
        </Stack>
      </Paper>
    </UnstyledButton>
  );
}

/** Dashboard row: the next GST returns, tax payments and year end, with how many days are left. */
export function KeyDates() {
  const business = useCurrentBusiness();
  const dates = useKeyDates(business.id);
  return (
    <SectionCard title="Key dates" description="What’s coming up with IRD and your financial year, and how long you have.">
      {dates.isError ? (
        <ErrorState error={dates.error} onRetry={() => void dates.refetch()} />
      ) : (
        <SimpleGrid cols={{ base: 1, xs: 2, md: 3, xl: 6 }} spacing="sm">
          {dates.isPending
            ? Array.from({ length: 3 }, (_, i) => <Skeleton key={i} height={110} radius="md" />)
            : dates.data.items.map((item) => (
                <KeyDateBox key={`${item.kind}-${item.date}-${item.detail}`} item={item} currency={business.currency} />
              ))}
        </SimpleGrid>
      )}
    </SectionCard>
  );
}
