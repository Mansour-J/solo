import { Anchor, Badge, Group, Paper, Stack, Table, Text, UnstyledButton } from '@mantine/core';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import type { Insight } from './api';
import { formatFigure, TONE_COLOURS } from './format';

const TONE_WORDS = { positive: 'Good', negative: 'Down', warning: 'Watch', neutral: null } as const;

/** One insight: its figure, what it means, how it was worked out, and the figures behind it. */
export function InsightCard({ insight, currency }: { insight: Insight; currency: string }) {
  const [open, setOpen] = useState(false);
  const toneWord = TONE_WORDS[insight.tone];
  return (
    <Paper withBorder radius="lg" p="md" h="100%">
      <Stack gap="xs" h="100%">
        <Group justify="space-between" align="flex-start" gap="xs" wrap="nowrap">
          <Text size="sm" fw={600} c="dimmed">
            {insight.title}
          </Text>
          <Group gap={4} wrap="nowrap" style={{ flexShrink: 0 }}>
            {insight.estimate && (
              <Badge size="xs" variant="outline" color="gray">
                Estimate
              </Badge>
            )}
            {toneWord && (
              <Badge size="xs" variant="light" color={TONE_COLOURS[insight.tone]}>
                {toneWord}
              </Badge>
            )}
          </Group>
        </Group>
        {insight.figure && (
          <Text fz={24} fw={650} className="tabular" lh={1.2}>
            {formatFigure(insight.figure, currency)}
          </Text>
        )}
        <Text size="sm">{insight.statement}</Text>
        <Text size="xs" c="dimmed">
          {insight.explanation}
        </Text>
        <Group gap="md" mt="auto" pt={4}>
          {insight.details.length > 0 && (
            <UnstyledButton onClick={() => setOpen((o) => !o)} aria-expanded={open}>
              <Group gap={4}>
                {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                <Text size="xs" c="brand">
                  {open ? 'Hide the figures' : 'Show the figures'}
                </Text>
              </Group>
            </UnstyledButton>
          )}
          {insight.link && (
            <Anchor component={Link} to={insight.link} size="xs">
              See the records
            </Anchor>
          )}
        </Group>
        {open && (
          <Table withRowBorders={false} verticalSpacing={2}>
            <Table.Tbody>
              {insight.details.map((d) => (
                <Table.Tr key={d.label}>
                  <Table.Td px={0}>
                    <Text size="xs">{d.label}</Text>
                  </Table.Td>
                  <Table.Td px={0} ta="right">
                    <Text size="xs" className="tabular">
                      {formatFigure(d.figure, currency)}
                    </Text>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        )}
      </Stack>
    </Paper>
  );
}
