import { Anchor, Box, Button, Container, Group, Paper, Stack, Stepper, Text, Title } from '@mantine/core';
import { useForm } from '@mantine/form';
import type { Business } from '@solo/api-client';
import { zodValidate } from '@/lib/forms';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { useIsMobile } from '@/hooks/useIsMobile';
import { Wordmark } from '@/components/Logo';
import { useCurrentUser } from '@/features/account/api';
import { businessKeys, useBusinesses } from '@/features/businesses/api';
import { ContactFields, FinancialFields, IdentityFields, TaxFields } from '@/features/businesses/BusinessFields';
import { businessSchema, emptyBusinessForm, toBusinessInput, type BusinessFormValues } from '@/features/businesses/businessForm';
import { LogoUploader } from '@/features/businesses/LogoUploader';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useSessionStore } from '@/stores/session';

const STEPS: { label: string; description: string; fields: string[] }[] = [
  { label: 'Business', description: 'Name and type', fields: ['name', 'trading_name', 'business_type'] },
  { label: 'Contact', description: 'Optional', fields: ['email', 'phone'] },
  { label: 'Tax', description: 'GST and NZBN', fields: ['nzbn', 'gst_number'] },
  { label: 'Financial year', description: 'Currency and dates', fields: [] },
  { label: 'Logo', description: 'Optional', fields: [] },
];

/** First-run setup: create a business in a few short steps. Optional details can be added later. */
export function OnboardingPage() {
  const user = useCurrentUser();
  const navigate = useNavigate();
  const { data: existing } = useBusinesses();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const isMobile = useIsMobile();
  const [step, setStep] = useState(0);
  const [created, setCreated] = useState<Business | null>(null);
  const form = useForm<BusinessFormValues>({
    initialValues: { ...emptyBusinessForm, trading_name: `${user.first_name} ${user.last_name}`.trim() },
    validate: zodValidate<BusinessFormValues>(businessSchema),
  });

  const create = useApiMutation(
    (values: BusinessFormValues) => unwrap(api.POST('/api/v1/businesses', { body: toBusinessInput(values, true) })),
    {
      form,
      invalidate: [businessKeys.all],
      onSuccess: (business) => {
        selectBusiness(business.id);
        setCreated(business);
        setStep(4);
      },
      onError: (error) => {
        // Jump back to the first step with a field error.
        const index = STEPS.findIndex((s) => s.fields.some((f) => Object.keys(error.fields).some((k) => k === f || k.startsWith(`${f}.`))));
        if (index >= 0) setStep(index);
      },
    },
  );

  const next = () => {
    const fields = STEPS[step]?.fields ?? [];
    const invalid = fields.some((field) => form.validateField(field).hasError);
    if (invalid) return;
    if (step === 3) create.mutate(form.getValues());
    else setStep((s) => s + 1);
  };

  const hasOtherBusinesses = (existing?.length ?? 0) > 0;
  const hasDemo = existing?.some((b) => b.is_demo) ?? false;

  return (
    <Box mih="100dvh" bg="var(--app-surface-muted)">
      <Group justify="space-between" px={{ base: 'md', sm: 'xl' }} py="md">
        <Wordmark />
        {!created && (
          <Group gap="md">
            {!hasDemo && (
              <Anchor size="sm" onClick={() => navigate('/welcome')}>
                Explore the demo first
              </Anchor>
            )}
            {hasOtherBusinesses && (
              <Anchor size="sm" onClick={() => navigate('/')}>
                Cancel
              </Anchor>
            )}
          </Group>
        )}
      </Group>
      <Container size={720} pb="xl">
        <Stack gap="lg">
          <Stack gap={4}>
            <Title order={1}>
              {created
                ? 'Your business is ready'
                : existing?.some((b) => !b.is_demo)
                  ? 'Add a business'
                  : user.first_name
                    ? `Kia ora, ${user.first_name}`
                    : 'Kia ora'}
            </Title>
            <Text c="dimmed">
              {created
                ? 'Add your logo now, or later from Settings.'
                : 'Tell us a little about your business. You can skip anything optional and finish it later in Settings.'}
            </Text>
          </Stack>
          <Paper withBorder p={{ base: 'md', sm: 'xl' }} radius="lg" bg="var(--app-surface-raised)">
            {isMobile && (
              <Text size="sm" c="dimmed" mb="sm">
                Step {step + 1} of {STEPS.length}: <strong>{STEPS[step]?.label}</strong>
              </Text>
            )}
            <Stepper active={step} size="sm" allowNextStepsSelect={false} onStepClick={(s) => !created && s < step && setStep(s)}>
              <Stepper.Step label={isMobile ? undefined : STEPS[0]?.label} description={isMobile ? undefined : STEPS[0]?.description}>
                <IdentityFields form={form} />
              </Stepper.Step>
              <Stepper.Step label={isMobile ? undefined : STEPS[1]?.label} description={isMobile ? undefined : STEPS[1]?.description}>
                <ContactFields form={form} />
              </Stepper.Step>
              <Stepper.Step label={isMobile ? undefined : STEPS[2]?.label} description={isMobile ? undefined : STEPS[2]?.description}>
                <TaxFields form={form} />
              </Stepper.Step>
              <Stepper.Step label={isMobile ? undefined : STEPS[3]?.label} description={isMobile ? undefined : STEPS[3]?.description}>
                <FinancialFields form={form} />
              </Stepper.Step>
              <Stepper.Step label={isMobile ? undefined : STEPS[4]?.label} description={isMobile ? undefined : STEPS[4]?.description}>
                {created && <LogoUploader businessId={created.id} hasLogo={created.has_logo} />}
              </Stepper.Step>
            </Stepper>
            <Group justify="space-between" mt="xl">
              {created ? (
                <span />
              ) : (
                <Button variant="default" onClick={() => setStep((s) => s - 1)} disabled={step === 0 || create.isPending}>
                  Back
                </Button>
              )}
              {created ? (
                <Button onClick={() => navigate('/', { replace: true })}>Go to dashboard</Button>
              ) : (
                <Group gap="xs">
                  {(step === 1 || step === 2) && (
                    <Button variant="subtle" onClick={() => setStep((s) => s + 1)}>
                      Skip for now
                    </Button>
                  )}
                  <Button onClick={next} loading={create.isPending}>
                    {step === 3 ? 'Create business' : 'Continue'}
                  </Button>
                </Group>
              )}
            </Group>
          </Paper>
        </Stack>
      </Container>
    </Box>
  );
}
