import {
  ActionIcon,
  Alert,
  Box,
  Button,
  Divider,
  Group,
  Paper,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  Textarea,
  TextInput,
  Tooltip,
} from '@mantine/core';
import { DateInput } from '@mantine/dates';
import type { UseFormReturnType } from '@mantine/form';
import { Plus, Trash2 } from 'lucide-react';
import { MoneyInput, MoneyText } from '@/components/Money';
import { SectionCard } from '@/components/States';
import { CustomerSelect } from '@/features/customers/CustomerSelect';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat } from '@/lib/format';
import { errorMessage } from '@/services/api/errors';
import { emptyLine, TAX_TREATMENTS, type DocumentValues } from './documentForm';
import { useCalculation } from './useCalculation';

interface Props {
  kind: 'invoice' | 'quote';
  form: UseFormReturnType<DocumentValues>;
  businessId: string;
  currency: string;
  gstRegistered: boolean;
  /** Issued invoices can't change customer once payments exist. */
  customerLocked?: boolean;
}

/** Invoice and quote editor: customer and dates, line items and live totals calculated by the API. */
export function DocumentEditor({ kind, form, businessId, currency, gstRegistered, customerLocked }: Props) {
  const isMobile = useIsMobile();
  const values = form.getValues();
  const { calculation, byKey, isFetching, error } = useCalculation(businessId, values.issue_date, values.prices_include_gst, values.lines);
  const lineError = form.errors.lines;

  return (
    <Stack gap="lg">
      <SectionCard>
        <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }}>
          <CustomerSelect
            label="Customer"
            required
            disabled={customerLocked}
            value={values.customer_id}
            onChange={(id) => form.setFieldValue('customer_id', id)}
            error={form.errors.customer_id}
          />
          <DateInput
            label={kind === 'invoice' ? 'Issue date' : 'Quote date'}
            required
            valueFormat={dateInputFormat()}
            {...form.getInputProps('issue_date')}
          />
          <DateInput
            label={kind === 'invoice' ? 'Due date' : 'Valid until'}
            placeholder={kind === 'invoice' ? 'From payment terms' : 'From quote validity'}
            clearable
            valueFormat={dateInputFormat()}
            minDate={values.issue_date || undefined}
            {...form.getInputProps('end_date')}
          />
          <TextInput label="Reference" placeholder="PO number, project…" {...form.getInputProps('reference')} />
        </SimpleGrid>
        {gstRegistered && (
          <Switch
            mt="md"
            label="Prices include GST"
            description="Turn on if the prices you enter already include GST."
            {...form.getInputProps('prices_include_gst', { type: 'checkbox' })}
          />
        )}
      </SectionCard>

      <SectionCard title="Items">
        <Stack gap="sm">
          {!isMobile && (
            <Group gap="xs" wrap="nowrap" px={4} c="dimmed">
              <Box w={18} />
              <Text size="xs" fw={600} style={{ flex: 1 }}>
                Description
              </Text>
              <Text size="xs" fw={600} w={80}>
                Qty
              </Text>
              <Text size="xs" fw={600} w={130}>
                {values.prices_include_gst ? 'Price incl. GST' : 'Price'}
              </Text>
              <Text size="xs" fw={600} w={80}>
                Discount %
              </Text>
              {gstRegistered && (
                <Text size="xs" fw={600} w={120}>
                  GST
                </Text>
              )}
              <Text size="xs" fw={600} w={110} ta="right">
                Amount
              </Text>
              <Box w={28} />
            </Group>
          )}
          {values.lines.map((line, i) => {
            const calculated = byKey.get(line.key);
            const fields = (
              <>
                <Textarea
                  aria-label={`Description, line ${i + 1}`}
                  placeholder="What you did or sold"
                  autosize
                  minRows={1}
                  style={{ flex: 1 }}
                  {...form.getInputProps(`lines.${i}.description`)}
                />
                <TextInput
                  aria-label={`Quantity, line ${i + 1}`}
                  inputMode="decimal"
                  w={isMobile ? undefined : 80}
                  {...form.getInputProps(`lines.${i}.quantity`)}
                />
                <MoneyInput
                  aria-label={`Price, line ${i + 1}`}
                  w={isMobile ? undefined : 130}
                  currency={currency}
                  value={line.unit_price}
                  onChange={(v) => form.setFieldValue(`lines.${i}.unit_price`, v)}
                  error={form.errors[`lines.${i}.unit_price`]}
                />
                <TextInput
                  aria-label={`Discount percent, line ${i + 1}`}
                  inputMode="decimal"
                  w={isMobile ? undefined : 80}
                  {...form.getInputProps(`lines.${i}.discount_percent`)}
                />
                {gstRegistered && (
                  <Select
                    aria-label={`GST, line ${i + 1}`}
                    w={isMobile ? undefined : 120}
                    data={TAX_TREATMENTS}
                    {...form.getInputProps(`lines.${i}.tax_treatment`)}
                  />
                )}
              </>
            );
            const remove = (
              <Tooltip label="Remove line">
                <ActionIcon
                  aria-label={`Remove line ${i + 1}`}
                  disabled={values.lines.length === 1}
                  onClick={() => form.removeListItem('lines', i)}
                >
                  <Trash2 size={16} />
                </ActionIcon>
              </Tooltip>
            );
            const amount = calculated ? (
              <MoneyText value={calculated.net_amount} currency={currency} fw={500} />
            ) : (
              <Text c="dimmed">—</Text>
            );
            return isMobile ? (
              <Paper key={line.key} withBorder p="sm" radius="md">
                <Stack gap="xs">
                  <Group justify="space-between">
                    <Text size="xs" c="dimmed">
                      Line {i + 1}
                    </Text>
                    {remove}
                  </Group>
                  {fields}
                  <Group justify="space-between">
                    <Text size="sm" c="dimmed">
                      Amount
                    </Text>
                    {amount}
                  </Group>
                </Stack>
              </Paper>
            ) : (
              <Group key={line.key} gap="xs" wrap="nowrap" align="flex-start">
                <Text w={18} pt={8} size="xs" c="dimmed" ta="right" aria-hidden>
                  {i + 1}
                </Text>
                {fields}
                <Box w={110} pt={8} ta="right">
                  {amount}
                </Box>
                <Box pt={4}>{remove}</Box>
              </Group>
            );
          })}
          {typeof lineError === 'string' && (
            <Text c="red" size="sm">
              {lineError}
            </Text>
          )}
          <Group>
            <Button
              variant="light"
              size="xs"
              leftSection={<Plus size={14} />}
              onClick={() => form.insertListItem('lines', emptyLine(gstRegistered))}
            >
              Add line
            </Button>
          </Group>
          <Divider />
          <Group justify="flex-end">
            <Stack gap={6} w={{ base: '100%', sm: 300 }}>
              {error ? (
                <Alert color="yellow" variant="light">
                  {errorMessage(error)}
                </Alert>
              ) : null}
              <Group justify="space-between">
                <Text c="dimmed">Subtotal</Text>
                {calculation ? <MoneyText value={calculation.totals.subtotal} currency={currency} /> : <Text c="dimmed">—</Text>}
              </Group>
              {(gstRegistered || (calculation?.totals.gst_total ?? 0) > 0) && (
                <Group justify="space-between">
                  <Text c="dimmed">GST</Text>
                  {calculation ? <MoneyText value={calculation.totals.gst_total} currency={currency} /> : <Text c="dimmed">—</Text>}
                </Group>
              )}
              <Group justify="space-between">
                <Text fw={700}>Total {currency}</Text>
                {calculation ? (
                  <MoneyText value={calculation.totals.total} currency={currency} fw={700} size="lg" />
                ) : (
                  <Text c="dimmed">—</Text>
                )}
              </Group>
              <Text size="xs" c="dimmed" ta="right" h={16}>
                {isFetching
                  ? 'Updating totals…'
                  : calculation && !calculation.gst_registered && gstRegistered
                    ? 'No GST: before your registration date'
                    : ''}
              </Text>
            </Stack>
          </Group>
        </Stack>
      </SectionCard>

      <SectionCard title="Notes">
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <Textarea
            label={kind === 'invoice' ? 'Message to the customer' : 'Notes'}
            autosize
            minRows={3}
            {...form.getInputProps('notes')}
          />
          <Textarea label="Footer" description="Small print at the bottom." autosize minRows={3} {...form.getInputProps('footer')} />
        </SimpleGrid>
      </SectionCard>
    </Stack>
  );
}
