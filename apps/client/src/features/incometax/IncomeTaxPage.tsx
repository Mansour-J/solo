import { Badge, Button, Group, Modal, Paper, Select, SimpleGrid, Stack, Table, Text } from '@mantine/core';
import { Receipt } from 'lucide-react';
import { useState } from 'react';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { MovementForm } from '@/features/yearend/MovementsPanel';
import { formatDate } from '@/lib/format';
import { AccLevies } from './AccLevies';
import { PAYMENT_STATUS, percent, useIncomeTax, type TaxPayment, type TaxYear } from './api';

function StatusBadge({ status }: { status: TaxPayment['status'] }) {
  return (
    <Badge variant="light" color={PAYMENT_STATUS[status].color}>
      {PAYMENT_STATUS[status].label}
    </Badge>
  );
}

function Figure({ label, value, currency, strong }: { label: string; value: number; currency: string; strong?: boolean }) {
  return (
    <Group justify="space-between" wrap="nowrap">
      <Text size="sm" fw={strong ? 600 : undefined}>
        {label}
      </Text>
      <MoneyText value={value} currency={currency} size="sm" fw={strong ? 700 : undefined} />
    </Group>
  );
}

function YearEstimate({ year, currency }: { year: TaxYear; currency: string }) {
  const rit = year.residual_income_tax;
  return (
    <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
      <Stack gap={6}>
        <Figure label="Income" value={year.income} currency={currency} />
        <Figure label="Deductible expenses" value={-year.deductible_expenses} currency={currency} />
        <Figure label="Taxable income" value={year.taxable_income} currency={currency} strong />
        <Figure label="Income tax" value={year.income_tax} currency={currency} />
        <Figure label="Tax withheld by customers" value={-year.withholding_tax} currency={currency} />
        <Figure label={rit < 0 ? 'Refund (residual income tax)' : 'Residual income tax'} value={Math.abs(rit)} currency={currency} strong />
      </Stack>
      <Table verticalSpacing={4} fz="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Income band</Table.Th>
            <Table.Th ta="right">Rate</Table.Th>
            <Table.Th ta="right">Income</Table.Th>
            <Table.Th ta="right">Tax</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {year.bands.map((b) => (
            <Table.Tr key={b.from}>
              <Table.Td>
                <MoneyText value={b.from} currency={currency} size="sm" />
                {b.to === null || b.to === undefined ? (
                  ' and over'
                ) : (
                  <>
                    {' '}
                    – <MoneyText value={b.to} currency={currency} size="sm" />
                  </>
                )}
              </Table.Td>
              <Table.Td ta="right">{percent(b.rate)}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={b.income} currency={currency} size="sm" c={b.income ? undefined : 'dimmed'} />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={b.tax} currency={currency} size="sm" c={b.tax ? undefined : 'dimmed'} />
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </SimpleGrid>
  );
}

function Payments({ year, currency, onPay }: { year: TaxYear; currency: string; onPay: (p: TaxPayment) => void }) {
  return (
    <Table.ScrollContainer minWidth={640}>
      <Table verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Payment</Table.Th>
            <Table.Th>Due</Table.Th>
            <Table.Th ta="right">Amount</Table.Th>
            <Table.Th ta="right">Paid</Table.Th>
            <Table.Th ta="right">Left</Table.Th>
            <Table.Th>Status</Table.Th>
            <Table.Th />
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {year.payments.map((p) => (
            <Table.Tr key={`${p.kind}-${p.due_date}`}>
              <Table.Td fw={500}>{p.kind === 'provisional' ? `Provisional tax, ${p.label.toLowerCase()}` : p.label}</Table.Td>
              <Table.Td>{formatDate(p.due_date)}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={Math.abs(p.amount)} currency={currency} size="sm" />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={p.settled} currency={currency} size="sm" c={p.settled ? undefined : 'dimmed'} />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={p.outstanding} currency={currency} size="sm" fw={p.outstanding ? 600 : undefined} />
              </Table.Td>
              <Table.Td>
                <StatusBadge status={p.status} />
              </Table.Td>
              <Table.Td ta="right">
                {(p.status === 'overdue' || p.status === 'upcoming') && p.amount > 0 && (
                  <Button size="compact-sm" variant="light" onClick={() => onPay(p)}>
                    Record payment
                  </Button>
                )}
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

/** Income tax, provisional tax and terminal tax for each income year, from the records. */
export function IncomeTaxPage() {
  const business = useCurrentBusiness();
  const tax = useIncomeTax();
  const [selected, setSelected] = useState<string | null>(null);
  const [paying, setPaying] = useState<TaxPayment | null>(null);

  let content;
  if (tax.isPending) content = <ListSkeleton rows={6} height={44} />;
  else if (tax.isError) content = <ErrorState error={tax.error} onRetry={() => void tax.refetch()} />;
  else if (tax.data.years.length === 0) {
    content = (
      <EmptyBlock icon={Receipt} title="Nothing recorded yet" description="Your income tax estimate appears once you record income." />
    );
  } else {
    const data = tax.data;
    const year = data.years.find((y) => y.label === selected) ?? data.years[0]!;
    const next = data.next_due;
    content = (
      <Stack gap="lg">
        <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
          <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
            <Text size="sm" c="dimmed">
              Next payment to IRD
            </Text>
            {next ? (
              <>
                <MoneyText value={next.outstanding} currency={business.currency} fw={700} size="xl" />
                <Text size="sm">
                  {next.kind === 'provisional' ? `Provisional tax, ${next.label.toLowerCase()}` : next.label}, due{' '}
                  {formatDate(next.due_date)}.
                </Text>
              </>
            ) : (
              <Text size="sm" mt={4}>
                Nothing due.
              </Text>
            )}
          </Paper>
          <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
            <Text size="sm" c="dimmed">
              Overdue
            </Text>
            <MoneyText value={data.overdue} currency={business.currency} fw={700} size="xl" c={data.overdue ? 'red' : undefined} />
            <Text size="sm">Income tax past its due date. IRD charges interest and late payment penalties on it.</Text>
          </Paper>
          <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
            <Text size="sm" c="dimmed">
              IRD owes you
            </Text>
            <MoneyText value={data.refunds_due + data.credit} currency={business.currency} fw={700} size="xl" />
            <Text size="sm">Refunds where more was withheld or paid than the tax.</Text>
          </Paper>
        </SimpleGrid>
        <SectionCard
          title={`Income tax estimate, ${year.label}`}
          description={`${formatDate(year.start)} to ${formatDate(year.end)}${year.ended ? '' : ', so far'}, on the ${data.basis} basis.`}
          actions={
            <Select
              aria-label="Income year"
              size="xs"
              w={130}
              allowDeselect={false}
              value={year.label}
              data={data.years.map((y) => y.label)}
              onChange={setSelected}
            />
          }
        >
          <Stack gap="lg">
            <YearEstimate year={year} currency={business.currency} />
            {year.provisional_tax !== null && year.provisional_tax !== undefined ? (
              <Text size="sm">
                Provisional tax for {year.label}: <MoneyText value={year.provisional_tax} currency={business.currency} size="sm" fw={600} />
                , last year’s residual income tax plus 5% (IRD’s standard option), because it was over $5,000.
              </Text>
            ) : (
              <Text size="sm" c="dimmed">
                No provisional tax for {year.label}: the year before’s residual income tax was $5,000 or less, or isn’t recorded here.
              </Text>
            )}
            <Payments year={year} currency={business.currency} onPay={setPaying} />
          </Stack>
        </SectionCard>
        <AccLevies year={year} settings={data.acc_settings} />
        <Text size="xs" c="dimmed">
          An estimate from your business records only: other income (wages, interest, rent), tax credits and losses aren’t included.
          Payments recorded under Year end › Money in and out (income tax paid to IRD) settle the earliest due date first. Under IRD’s
          standard option, when your residual income tax is under $60,000 IRD doesn’t charge interest before the terminal tax date (the safe
          harbour), though late payment penalties can still apply. Terminal tax is due 7 February (7 April if you have a tax agent); dates
          on a weekend or public holiday move to the next working day. Check with IRD or your accountant before you rely on these figures.
        </Text>
      </Stack>
    );
  }

  return (
    <Page title="Income tax" description="Your income tax estimate, provisional tax and when to pay IRD.">
      {content}
      <Modal opened={paying !== null} onClose={() => setPaying(null)} title="Record an income tax payment">
        {paying && (
          <MovementForm
            movement={null}
            preset={{
              kind: 'income_tax_payment',
              amount: paying.outstanding,
              description: paying.kind === 'provisional' ? `Provisional tax, ${paying.label.toLowerCase()}` : paying.label,
            }}
            onDone={() => setPaying(null)}
          />
        )}
      </Modal>
    </Page>
  );
}
