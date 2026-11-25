import { Modal, Stack, Table, Text } from '@mantine/core';
import { MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { periodLabel, useGstReturnBoxes, type GstReturnBoxes } from './api';

const BOXES: { box: string; label: string; value: (b: GstReturnBoxes) => number; total?: boolean }[] = [
  { box: '5', label: 'Total sales and income, including GST and zero-rated supplies', value: (b) => b.box5_sales },
  { box: '6', label: 'Zero-rated supplies', value: (b) => b.box6_zero_rated },
  { box: '7', label: 'Box 5 less box 6', value: (b) => b.box7 },
  { box: '8', label: 'GST on sales and income', value: (b) => b.box8_gst },
  { box: '9', label: 'Debit adjustments', value: (b) => b.box9_debit_adjustments },
  { box: '10', label: 'Total GST collected (8 + 9)', value: (b) => b.box10, total: true },
  { box: '11', label: 'Purchases and expenses, including GST, with GST claimed', value: (b) => b.box11_purchases },
  { box: '12', label: 'GST on purchases and expenses', value: (b) => b.box12_gst },
  { box: '13', label: 'Credit adjustments', value: (b) => b.box13_credit_adjustments },
  { box: '14', label: 'Total GST credit (12 + 13)', value: (b) => b.box14, total: true },
];

function Figures({ end }: { end: string }) {
  const business = useCurrentBusiness();
  const boxes = useGstReturnBoxes(end);
  if (boxes.isPending) return <ListSkeleton rows={6} />;
  if (boxes.isError) return <ErrorState error={boxes.error} onRetry={() => void boxes.refetch()} />;
  const b = boxes.data;
  return (
    <Stack gap="sm">
      <Text size="sm">
        {periodLabel(b.start, b.end)}, due {formatDate(b.due_date)}. Copy these into your return in myIR.
      </Text>
      <Table verticalSpacing={6}>
        <Table.Tbody>
          {BOXES.map((row) => (
            <Table.Tr key={row.box}>
              <Table.Td w={64} c="dimmed" style={{ whiteSpace: 'nowrap' }}>
                Box {row.box}
              </Table.Td>
              <Table.Td fw={row.total ? 600 : undefined}>{row.label}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={row.value(b)} currency={business.currency} size="sm" fw={row.total ? 600 : undefined} />
              </Table.Td>
            </Table.Tr>
          ))}
          <Table.Tr>
            <Table.Td c="dimmed" style={{ whiteSpace: 'nowrap' }}>
              Box 15
            </Table.Td>
            <Table.Td fw={700}>{b.box15 >= 0 ? 'GST to pay' : 'GST refund'}</Table.Td>
            <Table.Td ta="right">
              <MoneyText value={Math.abs(b.box15)} currency={business.currency} fw={700} />
            </Table.Td>
          </Table.Tr>
        </Table.Tbody>
      </Table>
      <Text size="xs" c="dimmed">
        Worked out from your records on your GST basis. Adjustments include entertainment, GST change of use and bad debts written off.
        Check the figures before you file: they’re for your information.
      </Text>
    </Stack>
  );
}

/** The figures for IRD's GST return (boxes 5 to 15) for one period. */
export function ReturnFigures({ end, onClose }: { end: string | null; onClose: () => void }) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={end !== null} onClose={onClose} title="Return figures" size="lg" fullScreen={isMobile}>
      {end && <Figures end={end} />}
    </Modal>
  );
}
