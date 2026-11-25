import { Badge, Button, Group, Modal, Paper, SimpleGrid, Stack, Table, Text } from '@mantine/core';
import { Landmark } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { MovementForm } from '@/features/yearend/MovementsPanel';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { periodEndMonths, periodLabel, STATUS, useGstReturns, type GstReturn, type GstReturns } from './api';
import { ReturnFigures } from './ReturnFigures';

const BASIS_LABELS: Record<GstReturns['gst_basis'], string> = {
  payments: 'payments basis',
  invoice: 'invoice basis',
  hybrid: 'hybrid basis',
};

function StatusBadge({ status }: { status: GstReturn['status'] }) {
  return (
    <Badge variant="light" color={STATUS[status].color}>
      {STATUS[status].label}
    </Badge>
  );
}

/** What is left on a return: to pay, or (for a refund) to receive. */
function toPay(r: GstReturn) {
  return r.net > 0 ? r.outstanding : 0;
}

function Summary({ data, currency }: { data: GstReturns; currency: string }) {
  const next = data.next_due;
  return (
    <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
      <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
        <Text size="sm" c="dimmed">
          Next return
        </Text>
        {next ? (
          <Stack gap={4} mt={4}>
            <Group gap="xs" justify="space-between">
              <MoneyText value={Math.max(next.amount, 0)} currency={currency} fw={700} size="xl" />
              <StatusBadge status={next.status} />
            </Group>
            <Text size="sm">
              {next.status === 'in_progress' ? 'So far, for the period ending ' : 'For the period ending '}
              {formatDate(next.end)}. Due {formatDate(next.due_date)}.
            </Text>
            {next.amount < 0 && (
              <Text size="sm" c="dimmed">
                A refund of <MoneyText value={-next.amount} currency={currency} size="sm" /> so far.
              </Text>
            )}
          </Stack>
        ) : (
          <Text size="sm" mt={4}>
            Nothing recorded yet.
          </Text>
        )}
      </Paper>
      <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
        <Text size="sm" c="dimmed">
          To pay IRD now
        </Text>
        <MoneyText value={data.owing_now} currency={currency} fw={700} size="xl" c={data.overdue > 0 ? 'red' : undefined} />
        <Text size="sm" mt={4}>
          {data.overdue > 0 ? (
            <>
              <MoneyText value={data.overdue} currency={currency} size="sm" fw={600} /> of it is past its due date.
            </>
          ) : (
            'For returns that have ended. Nothing is overdue.'
          )}
        </Text>
      </Paper>
      <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
        <Text size="sm" c="dimmed">
          IRD owes you
        </Text>
        <MoneyText value={data.refunds_due + data.credit} currency={currency} fw={700} size="xl" />
        <Text size="sm" mt={4}>
          {data.credit > 0
            ? 'Refunds due and payments beyond what you owe.'
            : 'Refunds due on returns where you claimed more than you collected.'}
        </Text>
      </Paper>
    </SimpleGrid>
  );
}

function ReturnsTable({
  rows,
  currency,
  onPay,
  onFigures,
}: {
  rows: GstReturn[];
  currency: string;
  onPay: (r: GstReturn) => void;
  onFigures: (r: GstReturn) => void;
}) {
  const hasAdjustments = rows.some((r) => r.adjustments !== 0);
  return (
    <Table.ScrollContainer minWidth={900}>
      <Table highlightOnHover verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Period</Table.Th>
            <Table.Th>Due</Table.Th>
            <Table.Th ta="right">GST collected</Table.Th>
            <Table.Th ta="right">GST claimable</Table.Th>
            {hasAdjustments && <Table.Th ta="right">Adjustments</Table.Th>}
            <Table.Th ta="right">To pay</Table.Th>
            <Table.Th ta="right">Paid</Table.Th>
            <Table.Th ta="right">Left to pay</Table.Th>
            <Table.Th>Status</Table.Th>
            <Table.Th />
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((r) => (
            <Table.Tr key={r.end}>
              <Table.Td fw={500}>{periodLabel(r.start, r.end)}</Table.Td>
              <Table.Td>{formatDate(r.due_date)}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={r.collected} currency={currency} size="sm" />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={r.claimable} currency={currency} size="sm" />
              </Table.Td>
              {hasAdjustments && (
                <Table.Td ta="right">
                  <MoneyText value={r.adjustments} currency={currency} size="sm" c={r.adjustments ? undefined : 'dimmed'} />
                </Table.Td>
              )}
              <Table.Td ta="right">
                {r.net < 0 ? (
                  <Text size="sm" c="dimmed">
                    Refund <MoneyText value={-r.net} currency={currency} size="sm" />
                  </Text>
                ) : (
                  <MoneyText value={r.net} currency={currency} size="sm" fw={600} />
                )}
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={r.settled} currency={currency} size="sm" c={r.settled ? undefined : 'dimmed'} />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText
                  value={toPay(r)}
                  currency={currency}
                  size="sm"
                  fw={toPay(r) ? 600 : undefined}
                  c={toPay(r) ? undefined : 'dimmed'}
                />
              </Table.Td>
              <Table.Td>
                <StatusBadge status={r.status} />
              </Table.Td>
              <Table.Td ta="right">
                <Group gap={6} justify="flex-end" wrap="nowrap">
                  <Button size="compact-sm" variant="subtle" onClick={() => onFigures(r)}>
                    Return figures
                  </Button>
                  {(r.status === 'to_file' || r.status === 'overdue') && (
                    <Button size="compact-sm" variant="light" onClick={() => onPay(r)}>
                      Record payment
                    </Button>
                  )}
                </Group>
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

function ReturnCards({
  rows,
  currency,
  onPay,
  onFigures,
}: {
  rows: GstReturn[];
  currency: string;
  onPay: (r: GstReturn) => void;
  onFigures: (r: GstReturn) => void;
}) {
  return (
    <Stack gap="xs">
      {rows.map((r) => (
        <Paper key={r.end} withBorder radius="md" p="sm">
          <Group justify="space-between" wrap="nowrap" align="flex-start">
            <div style={{ minWidth: 0 }}>
              <Text fw={600}>{periodLabel(r.start, r.end)}</Text>
              <Text size="xs" c="dimmed">
                Due {formatDate(r.due_date)} · collected <MoneyText value={r.collected} currency={currency} size="xs" />, claimable{' '}
                <MoneyText value={r.claimable} currency={currency} size="xs" />
              </Text>
            </div>
            <Stack gap={4} align="flex-end">
              {r.net < 0 ? (
                <Text size="sm">
                  Refund <MoneyText value={-r.net} currency={currency} size="sm" />
                </Text>
              ) : (
                <MoneyText value={r.status === 'paid' ? r.net : toPay(r) || r.net} currency={currency} fw={600} />
              )}
              <StatusBadge status={r.status} />
            </Stack>
          </Group>
          <Group mt="xs" gap="xs" grow>
            <Button size="xs" variant="default" onClick={() => onFigures(r)}>
              Return figures
            </Button>
            {(r.status === 'to_file' || r.status === 'overdue') && (
              <Button size="xs" variant="light" onClick={() => onPay(r)}>
                Record payment
              </Button>
            )}
          </Group>
        </Paper>
      ))}
    </Stack>
  );
}

/** Each GST return period: when it's due, what to pay IRD, and what has been paid. */
export function GstReturnsPage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const returns = useGstReturns();
  const [paying, setPaying] = useState<GstReturn | null>(null);
  const [figures, setFigures] = useState<string | null>(null);

  let content;
  if (returns.isPending) content = <ListSkeleton rows={6} height={44} />;
  else if (returns.isError) content = <ErrorState error={returns.error} onRetry={() => void returns.refetch()} />;
  else if (!returns.data.gst_registered) {
    content = (
      <EmptyBlock
        icon={Landmark}
        title="Not registered for GST"
        description="If you register, add your GST number, when you registered and how often you file in Settings. Your returns then appear here."
        action={
          <Button component={Link} to="/settings/business">
            Open business details
          </Button>
        }
      />
    );
  } else {
    const data = returns.data;
    const rows = [...data.returns].reverse();
    const bf = data.brought_forward;
    content = (
      <Stack gap="lg">
        <Summary data={data} currency={business.currency} />
        <SectionCard
          title="Returns"
          description={`${data.gst_period === 'monthly' ? 'Monthly' : data.gst_period === 'two_monthly' ? 'Two-monthly' : 'Six-monthly'} returns, periods ending ${periodEndMonths(data.gst_period, data.gst_period_end_month)}, on the ${BASIS_LABELS[data.gst_basis]}. Change them in Settings › Business details.`}
        >
          {rows.length === 0 ? (
            <EmptyBlock
              compact
              icon={Landmark}
              title="No GST yet"
              description="Returns appear once you record sales or expenses with GST."
            />
          ) : isMobile ? (
            <ReturnCards rows={rows} currency={business.currency} onPay={setPaying} onFigures={(r) => setFigures(r.end)} />
          ) : (
            <ReturnsTable rows={rows} currency={business.currency} onPay={setPaying} onFigures={(r) => setFigures(r.end)} />
          )}
          {bf && (
            <Text size="sm" c="dimmed" mt="sm">
              Brought forward from your opening balances on {formatDate(bf.as_at)}: {bf.amount >= 0 ? 'GST owed ' : 'a refund due of '}
              <MoneyText value={Math.abs(bf.amount)} currency={business.currency} size="sm" />, with{' '}
              <MoneyText value={bf.outstanding} currency={business.currency} size="sm" /> left.
            </Text>
          )}
          <Text size="xs" c="dimmed" mt="sm">
            Payments recorded under Year end › Money in and out (GST paid to IRD) settle the oldest returns first. Due dates are IRD’s; one
            falling on a weekend or a national public holiday moves to the next working day. If you file late, IRD may charge a late filing
            penalty ($50 a return on the payments basis, $250 on the invoice or hybrid basis) and late payment penalties and interest. These
            figures are for your information: check them before you file with IRD.
          </Text>
        </SectionCard>
      </Stack>
    );
  }

  return (
    <Page title="GST returns" description="Each GST period, when its return is due, and how much to pay IRD.">
      {content}
      <ReturnFigures end={figures} onClose={() => setFigures(null)} />
      <Modal opened={paying !== null} onClose={() => setPaying(null)} title="Record a GST payment">
        {paying && (
          <MovementForm
            movement={null}
            preset={{
              kind: 'gst_payment',
              amount: toPay(paying),
              description: `GST return ${periodLabel(paying.start, paying.end)}`,
            }}
            onDone={() => setPaying(null)}
          />
        )}
      </Modal>
    </Page>
  );
}
