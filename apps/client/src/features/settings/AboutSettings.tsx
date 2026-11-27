import type { ReactNode } from 'react';
import { Anchor, Button, Group, List, Stack, Table, Text, ThemeIcon } from '@mantine/core';
import { Check, Code2, ExternalLink, X } from 'lucide-react';
import { SectionCard } from '@/components/States';
import { APP_VERSION, BRAND } from '@/branding/brand';
import { openExternal } from '@/services/platform';

const LICENCE_TEXT_URL = `${BRAND.repositoryUrl}/blob/main/LICENSE`;

/** A link that opens in the system browser (the desktop app has no tabs of its own). */
function OutsideLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <Anchor
      href={href}
      onClick={(event) => {
        event.preventDefault();
        void openExternal(href);
      }}
    >
      {children}
    </Anchor>
  );
}

function Answer({ allowed }: { allowed: boolean }) {
  return (
    <ThemeIcon size={24} radius="xl" variant="light" color={allowed ? 'green' : 'red'} role="img" aria-label={allowed ? 'Yes' : 'No'}>
      {allowed ? <Check size={15} strokeWidth={2.5} /> : <X size={15} strokeWidth={2.5} />}
    </ThemeIcon>
  );
}

const LICENCE_ROWS: { what: string; allowed: boolean; note: string }[] = [
  { what: 'Use it for your business, for free', allowed: true, note: 'There’s no charge, subscription or limit on how long.' },
  { what: 'Read and inspect the code', allowed: true, note: 'All of it is public, so you can check what it does with your records.' },
  { what: 'Change it for yourself', allowed: true, note: 'Change anything you like for your own use.' },
  {
    what: 'Share copies, changed or not',
    allowed: true,
    note: 'Give anyone a copy, as long as they get the same licence and the code with it.',
  },
  { what: 'Charge for copies or for support', allowed: true, note: 'Allowed, with the code shared under the same licence.' },
  {
    what: 'Offer a changed version to others over a network while keeping the changes private',
    allowed: false,
    note: 'Anyone who uses it over a network must be able to get your changed code.',
  },
  {
    what: 'Share it under a different licence, or as closed source',
    allowed: false,
    note: 'Copies and changed versions stay under this licence.',
  },
  { what: 'Remove the copyright or licence notices', allowed: false, note: 'They stay with every copy.' },
  {
    what: 'Expect a warranty',
    allowed: false,
    note: `${BRAND.name} comes as it is. The author isn’t liable if something goes wrong, so check your figures before you file with IRD.`,
  },
];

/** Settings › Licence: what the licence lets you do, in a table. */
export function LicenceSettings() {
  return (
    <SectionCard
      title="Licence"
      description={`${BRAND.name} is free, open-source software under the ${BRAND.license} (AGPL-3.0). In plain words, this is what it allows.`}
    >
      <Stack gap="md">
        <Table verticalSpacing="sm">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Can you…</Table.Th>
              <Table.Th w={80} ta="center">
                Allowed
              </Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {LICENCE_ROWS.map((row) => (
              <Table.Tr key={row.what}>
                <Table.Td>
                  <Text size="sm" fw={500}>
                    {row.what}
                  </Text>
                  <Text size="sm" c="dimmed">
                    {row.note}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Group justify="center">
                    <Answer allowed={row.allowed} />
                  </Group>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
        <Text size="sm" c="dimmed">
          This is a summary, not legal advice. The full licence text is what counts:{' '}
          <OutsideLink href={LICENCE_TEXT_URL}>read the full licence</OutsideLink>.
        </Text>
      </Stack>
    </SectionCard>
  );
}

/** Settings › About: who makes the app, what it's for, and that the records never leave this computer. */
export function AboutSettings() {
  return (
    <Stack gap="lg">
      <SectionCard title={`About ${BRAND.name}`} description={`Version ${APP_VERSION}`}>
        <Stack gap="md">
          <Text>
            {BRAND.name} makes it easier for New Zealand sole traders to keep their books and work out their GST and income tax, without
            needing an accountant’s software.
          </Text>
          <Text>
            Made by <strong>{BRAND.author}</strong>.
          </Text>
          <Group>
            <Button
              variant="default"
              leftSection={<Code2 size={16} />}
              rightSection={<ExternalLink size={14} />}
              onClick={() => void openExternal(BRAND.repositoryUrl)}
            >
              View the code on GitHub
            </Button>
          </Group>
        </Stack>
      </SectionCard>
      <SectionCard title="Your privacy" description="Your records are yours, and they stay with you.">
        <List spacing="sm" icon={<Answer allowed />}>
          <List.Item>
            <strong>No analytics or tracking.</strong> {BRAND.name} never collects or sends data about you or how you use it.
          </List.Item>
          <List.Item>
            <strong>Everything stays on your own machine.</strong> Your books and receipts are kept on this computer; nothing is uploaded to{' '}
            {BRAND.author} or anyone else.
          </List.Item>
          <List.Item>
            <strong>The code is available to inspect.</strong> Anyone can read exactly what {BRAND.name} does on GitHub.
          </List.Item>
        </List>
        <Text size="sm" c="dimmed" mt="md">
          {BRAND.name} only goes online when you ask it to: email you send through your own email account (set up in Settings › Email),
          checking for updates when you choose <strong>Check for updates</strong> in Settings › Updates, and links you open in your browser.
        </Text>
      </SectionCard>
    </Stack>
  );
}
