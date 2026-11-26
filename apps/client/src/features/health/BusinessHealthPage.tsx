import { Accordion, Badge, Button, Group, Paper, SimpleGrid, Stack, Text, Title } from '@mantine/core';
import { ArrowRight, RefreshCw } from 'lucide-react';
import { Link } from 'react-router';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { formatDateTime } from '@/lib/format';
import { useBusinessHealth, type HealthArea, type HealthCheck, type HealthStatus } from './api';

const STATUS: Record<HealthStatus, { color: string; badge: string; heading: string; tile: string }> = {
  critical: { color: 'red', badge: 'Fix now', heading: 'Fix now', tile: 'To fix now' },
  warning: { color: 'orange', badge: 'Worth fixing', heading: 'Worth fixing', tile: 'Worth fixing' },
  recommendation: { color: 'blue', badge: 'Suggestion', heading: 'Suggestions', tile: 'Suggestions' },
  passed: { color: 'teal', badge: 'Passed', heading: 'Passed', tile: 'Passed' },
  not_applicable: { color: 'gray', badge: 'Doesn’t apply', heading: 'Doesn’t apply', tile: 'Doesn’t apply' },
};

const AREAS: Record<HealthArea, string> = {
  details: 'Business details',
  gst: 'GST',
  getting_paid: 'Getting paid',
  records: 'Records',
};

const PROBLEMS: HealthStatus[] = ['critical', 'warning', 'recommendation'];

function headline(count: (s: HealthStatus) => number): { title: string; color: string } {
  if (count('critical') > 0) return { title: 'Needs attention', color: 'red' };
  if (count('warning') > 0) return { title: 'A few things to look at', color: 'orange' };
  if (count('recommendation') > 0) return { title: 'In good shape', color: 'teal' };
  return { title: 'All set', color: 'teal' };
}

function Problem({ check }: { check: HealthCheck }) {
  const status = STATUS[check.status];
  return (
    <Paper withBorder radius="md" p="md" style={{ borderLeft: `3px solid var(--mantine-color-${status.color}-6)` }}>
      <Stack gap="xs">
        <Group justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
          <Stack gap={0} style={{ minWidth: 0 }}>
            <Text fw={600}>{check.title}</Text>
            <Text size="xs" c="dimmed">
              {AREAS[check.area]}
            </Text>
          </Stack>
          <Badge color={status.color} variant="light" style={{ flexShrink: 0 }}>
            {status.badge}
          </Badge>
        </Group>
        <Text size="sm">{check.finding}</Text>
        <Text size="sm" c="dimmed">
          <Text span fw={600} inherit>
            Why it matters:
          </Text>{' '}
          {check.why}
        </Text>
        {check.action && check.link && (
          <Group>
            <Button component={Link} to={check.link} size="xs" variant="light" rightSection={<ArrowRight size={14} />}>
              {check.action}
            </Button>
          </Group>
        )}
      </Stack>
    </Paper>
  );
}

function Settled({ status, checks }: { status: 'passed' | 'not_applicable'; checks: HealthCheck[] }) {
  if (checks.length === 0) return null;
  return (
    <Accordion.Item value={status}>
      <Accordion.Control>
        <Text fw={600}>
          {STATUS[status].heading} ({checks.length})
        </Text>
      </Accordion.Control>
      <Accordion.Panel>
        <Stack gap="xs">
          {checks.map((c) => (
            <Group key={c.key} justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
              <Stack gap={0} style={{ minWidth: 0 }}>
                <Text size="sm" fw={500}>
                  {c.title}
                </Text>
                <Text size="sm" c="dimmed">
                  {c.finding}
                </Text>
              </Stack>
              <Text size="xs" c="dimmed" style={{ flexShrink: 0 }}>
                {AREAS[c.area]}
              </Text>
            </Group>
          ))}
        </Stack>
      </Accordion.Panel>
    </Accordion.Item>
  );
}

/**
 * Settings › Business health: whether this business is set up to issue correct invoices, keep the
 * records IRD expects and get paid. System status covers whether the app itself works.
 */
export function BusinessHealthPage() {
  const health = useBusinessHealth();
  let content;
  if (health.isPending) content = <ListSkeleton rows={4} />;
  else if (health.isError) content = <ErrorState error={health.error} onRetry={() => void health.refetch()} />;
  else {
    const checks = health.data.checks;
    const of = (status: HealthStatus) => checks.filter((c) => c.status === status);
    const count = (status: HealthStatus) => of(status).length;
    const top = headline(count);
    const tiles: HealthStatus[] = ['critical', 'warning', 'recommendation', 'passed'];
    content = (
      <Stack gap="lg">
        <Stack gap="sm">
          <Group justify="space-between" align="baseline" gap="xs">
            <Title order={4} c={`${top.color}.7`}>
              {top.title}
            </Title>
            <Text size="xs" c="dimmed">
              Checked {formatDateTime(health.data.checked_at)}
            </Text>
          </Group>
          <SimpleGrid cols={{ base: 2, sm: 4 }} spacing="sm">
            {tiles.map((s) => (
              <Paper key={s} withBorder radius="md" p="sm">
                <Text size="xl" fw={700} c={count(s) > 0 ? `${STATUS[s].color}.7` : 'dimmed'}>
                  {count(s)}
                </Text>
                <Text size="sm" c="dimmed">
                  {STATUS[s].tile}
                </Text>
              </Paper>
            ))}
          </SimpleGrid>
        </Stack>
        {PROBLEMS.map((s) =>
          count(s) > 0 ? (
            <Stack key={s} gap="sm">
              <Text fw={600} size="sm" tt="uppercase" c="dimmed" style={{ letterSpacing: '0.04em' }}>
                {STATUS[s].heading}
              </Text>
              {of(s).map((c) => (
                <Problem key={c.key} check={c} />
              ))}
            </Stack>
          ) : null,
        )}
        {PROBLEMS.every((s) => count(s) === 0) && <Text size="sm">Everything that applies to your business is in place.</Text>}
        <Accordion variant="separated" multiple>
          <Settled status="passed" checks={of('passed')} />
          <Settled status="not_applicable" checks={of('not_applicable')} />
        </Accordion>
      </Stack>
    );
  }
  return (
    <SectionCard
      title="Business health"
      description="Checks this business is set up to issue correct invoices, keep the records IRD expects and get paid. System status checks the app itself."
      actions={
        <Button
          variant="default"
          size="xs"
          leftSection={<RefreshCw size={14} />}
          loading={health.isFetching}
          onClick={() => void health.refetch()}
        >
          Check again
        </Button>
      }
    >
      {content}
    </SectionCard>
  );
}
