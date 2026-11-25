import { Paper, Stack, Table, Text, Title } from '@mantine/core';
import { formatDate, formatMoney, formatNumber } from '@/lib/format';
import type { Report, ReportCell, ReportColumn, ReportSection } from './api';

function formatCell(cell: ReportCell, column: ReportColumn, currency: string): string {
  if (cell === null || cell === undefined) return '';
  switch (column.kind) {
    case 'money':
      return typeof cell === 'number' ? formatMoney(cell, currency) : String(cell);
    case 'count':
      return typeof cell === 'number' ? formatNumber(cell) : String(cell);
    case 'date':
      return typeof cell === 'string' ? formatDate(cell) : String(cell);
    default:
      return String(cell);
  }
}

const numeric = (column: ReportColumn) => column.kind === 'money' || column.kind === 'number' || column.kind === 'count';

function SectionTable({ section, currency }: { section: ReportSection; currency: string }) {
  const columns = section.columns;
  // Narrow tables fit a phone; wide ones scroll within their card.
  const minWidth = Math.max(280, columns.length * 105);
  return (
    <Paper withBorder radius="lg" p="md">
      <Stack gap={4} mb="sm">
        <Title order={3} size="h5">
          {section.title}
        </Title>
        {section.description && (
          <Text size="xs" c="dimmed">
            {section.description}
          </Text>
        )}
      </Stack>
      {section.rows.length === 0 && !section.totals ? (
        <Text size="sm" c="dimmed">
          Nothing in this period.
        </Text>
      ) : (
        <Table.ScrollContainer minWidth={minWidth}>
          <Table verticalSpacing={6}>
            <Table.Thead>
              <Table.Tr>
                {columns.map((c, i) => (
                  <Table.Th key={`${c.label}-${i}`} ta={numeric(c) ? 'right' : undefined}>
                    {c.label}
                  </Table.Th>
                ))}
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {section.rows.map((row, r) => (
                <Table.Tr key={r}>
                  {row.map((cell, i) => (
                    <Table.Td
                      key={i}
                      ta={numeric(columns[i]!) ? 'right' : undefined}
                      className={numeric(columns[i]!) ? 'tabular' : undefined}
                    >
                      {formatCell(cell, columns[i]!, currency)}
                    </Table.Td>
                  ))}
                </Table.Tr>
              ))}
            </Table.Tbody>
            {section.totals && (
              <Table.Tfoot>
                <Table.Tr>
                  {section.totals.map((cell, i) => (
                    <Table.Th
                      key={i}
                      ta={numeric(columns[i]!) ? 'right' : undefined}
                      className={numeric(columns[i]!) ? 'tabular' : undefined}
                    >
                      {formatCell(cell, columns[i]!, currency)}
                    </Table.Th>
                  ))}
                </Table.Tr>
              </Table.Tfoot>
            )}
          </Table>
        </Table.ScrollContainer>
      )}
    </Paper>
  );
}

/** Any report from the API: its context lines, sections as tables with totals, and notes. */
export function ReportView({ report }: { report: Report }) {
  return (
    <Stack gap="md">
      {(report.subtitle.length > 0 || report.basis) && (
        <Text size="sm" c="dimmed">
          {[...report.subtitle, report.basis ? `${report.basis === 'cash' ? 'Cash' : 'Accrual'} basis` : null].filter(Boolean).join(' · ')}
        </Text>
      )}
      {report.sections.map((section, i) => (
        <SectionTable key={`${section.title}-${i}`} section={section} currency={report.currency} />
      ))}
      {report.notes.length > 0 && (
        <Stack gap={2}>
          {report.notes.map((note) => (
            <Text key={note} size="xs" c="dimmed">
              {note}
            </Text>
          ))}
        </Stack>
      )}
    </Stack>
  );
}
