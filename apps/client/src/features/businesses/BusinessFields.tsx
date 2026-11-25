import { Radio, Select, SimpleGrid, Stack, Switch, Text, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import type { UseFormReturnType } from '@mantine/form';
import { dateInputFormat } from '@/lib/format';
import { BUSINESS_TYPES, DAY_OPTIONS, GST_BASES, GST_PERIODS, MONTH_OPTIONS, type BusinessFormValues } from './businessForm';

type Form = UseFormReturnType<BusinessFormValues>;

const CURRENCIES = ['NZD', 'AUD', 'USD', 'GBP', 'EUR', 'CAD', 'SGD', 'JPY'];

export function IdentityFields({ form }: { form: Form }) {
  return (
    <Stack gap="md">
      <TextInput label="Business name" description="Shown in the app and on your invoices." required {...form.getInputProps('name')} />
      <TextInput
        label="Your name or trading name"
        description="For example “Jane Smith” or “Jane Smith Design”. Optional."
        {...form.getInputProps('trading_name')}
      />
      <Select label="Business type" data={BUSINESS_TYPES} {...form.getInputProps('business_type')} />
    </Stack>
  );
}

export function ContactFields({ form }: { form: Form }) {
  return (
    <Stack gap="md">
      <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
        <TextInput label="Business email" type="email" description="Customers can reply to it." {...form.getInputProps('email')} />
        <TextInput label="Phone" type="tel" {...form.getInputProps('phone')} />
      </SimpleGrid>
      <TextInput label="Street address" {...form.getInputProps('address.line1')} />
      <TextInput label="Address line 2" aria-label="Address line 2" {...form.getInputProps('address.line2')} />
      <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
        <TextInput label="Suburb" {...form.getInputProps('address.suburb')} />
        <TextInput label="Town or city" {...form.getInputProps('address.city')} />
        <TextInput label="Region" {...form.getInputProps('address.region')} />
        <TextInput label="Postcode" {...form.getInputProps('address.postcode')} />
      </SimpleGrid>
    </Stack>
  );
}

export function TaxFields({ form }: { form: Form }) {
  const registered = form.getValues().gst_registered;
  return (
    <Stack gap="md">
      <TextInput
        label="NZBN"
        description="Your 13-digit New Zealand Business Number, if you have one."
        inputMode="numeric"
        {...form.getInputProps('nzbn')}
      />
      <Switch
        label="I’m registered for GST"
        description="You must register once your turnover is expected to exceed $60,000 in 12 months."
        {...form.getInputProps('gst_registered', { type: 'checkbox' })}
      />
      {registered && (
        <Stack gap="md" pl={{ base: 0, sm: 'xl' }}>
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
            <TextInput label="GST number" inputMode="numeric" placeholder="123-456-789" {...form.getInputProps('gst_number')} />
            <DateInput
              label="Registered from"
              valueFormat={dateInputFormat()}
              clearable
              maxDate={new Date()}
              {...form.getInputProps('gst_registered_from')}
            />
          </SimpleGrid>
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md">
            <Select
              label="GST returns"
              description="How often you file with IRD."
              allowDeselect={false}
              data={GST_PERIODS}
              {...form.getInputProps('gst_period')}
            />
            {form.getValues().gst_period !== 'monthly' && (
              <Select
                label="A return period ends in"
                description="Any month one of your periods ends in, e.g. March."
                allowDeselect={false}
                data={MONTH_OPTIONS}
                {...form.getInputProps('gst_period_end_month')}
              />
            )}
          </SimpleGrid>
          <Radio.Group label="GST accounting basis" {...form.getInputProps('gst_basis')}>
            <Stack gap="xs" mt="xs">
              {GST_BASES.map((basis) => (
                <Radio key={basis.value} value={basis.value} label={basis.label} description={basis.description} />
              ))}
            </Stack>
          </Radio.Group>
        </Stack>
      )}
    </Stack>
  );
}

export function FinancialFields({ form, currencyLocked = false }: { form: Form; currencyLocked?: boolean }) {
  return (
    <Stack gap="md">
      <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="md">
        <Select
          label="Currency"
          data={CURRENCIES}
          disabled={currencyLocked}
          description={currencyLocked ? 'Locked once you have financial records.' : undefined}
          {...form.getInputProps('currency')}
        />
        <Select label="Financial year starts" data={MONTH_OPTIONS} {...form.getInputProps('financial_year_start_month')} />
        <Select label="Day" data={DAY_OPTIONS} {...form.getInputProps('financial_year_start_day')} />
      </SimpleGrid>
      <Text size="xs" c="dimmed">
        The New Zealand tax year runs from 1 April to 31 March.
      </Text>
      <Radio.Group
        label="Default reporting basis"
        description="Every report shows which basis it uses."
        {...form.getInputProps('reporting_basis')}
      >
        <Stack gap="xs" mt="xs">
          <Radio value="accrual" label="Accrual" description="Income counts when you invoice; expenses when you incur them." />
          <Radio value="cash" label="Cash" description="Income counts when you’re paid; expenses when you pay." />
        </Stack>
      </Radio.Group>
    </Stack>
  );
}
