import { SimpleGrid, Text, TextInput, type TextInputProps } from '@mantine/core';
import { BRAND } from '@/branding/brand';

/** A rate per kilometre typed as a decimal string (e.g. "1.04"), sent to the API as typed. */
export function RatePerKmInput({ currency, ...props }: TextInputProps & { currency: string }) {
  return (
    <TextInput
      label="Rate per km"
      description={`In ${currency}, up to 4 decimal places.`}
      inputMode="decimal"
      autoComplete="off"
      rightSection={
        <Text size="xs" c="dimmed">
          /km
        </Text>
      }
      rightSectionWidth={40}
      maxLength={12}
      {...props}
    />
  );
}

/** Rates are the user's own figures: the app never supplies or suggests one. */
export function RateSourceNote() {
  return (
    <Text size="xs" c="dimmed">
      Rates are your own figures, for example the kilometre rate IRD publishes for the year. {BRAND.name} doesn’t supply them.
    </Text>
  );
}

/**
 * IRD's two tiers, optional: the rate above applies to a vehicle's first `limit` kilometres in the
 * financial year (business and private), and the tier 2 rate to business kilometres after that.
 */
export function TierInputs({
  currency,
  limitProps,
  rateProps,
}: {
  currency: string;
  limitProps: TextInputProps;
  rateProps: TextInputProps;
}) {
  return (
    <SimpleGrid cols={{ base: 1, sm: 2 }}>
      <TextInput
        label="Tier 1 limit (optional)"
        description="Kilometres a year at the rate above, such as IRD’s 14,000."
        inputMode="decimal"
        autoComplete="off"
        rightSection={
          <Text size="xs" c="dimmed">
            km
          </Text>
        }
        rightSectionWidth={36}
        maxLength={12}
        {...limitProps}
      />
      <RatePerKmInput
        currency={currency}
        label="Tier 2 rate (optional)"
        description="For business kilometres after the limit."
        {...rateProps}
      />
    </SimpleGrid>
  );
}
