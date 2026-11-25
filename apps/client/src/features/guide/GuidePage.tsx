import {
  Alert,
  Badge,
  Box,
  Button,
  Checkbox,
  Grid,
  Group,
  NavLink,
  Paper,
  Progress,
  Select,
  SimpleGrid,
  Stack,
  Text,
  ThemeIcon,
  Timeline,
  Title,
} from '@mantine/core';
import { useLocalStorage } from '@mantine/hooks';
import { ArrowRight, BookOpen, CircleCheck, Info, Lightbulb } from 'lucide-react';
import { Link } from 'react-router';
import { Page } from '@/components/Page';
import { BRAND } from '@/branding/brand';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { emphasis, GLOSSARY, GUIDE_CHAPTERS, type GuideChapter, type GuideStep } from './content';

const GLOSSARY_ID = 'glossary';
const SCROLL_MARGIN = 'calc(3.5rem + var(--app-safe-top) + var(--mantine-spacing-md))';

function scrollToSection(id: string) {
  document.getElementById(`guide-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

/** Text with the app's own words (`**label**`) in bold. */
function Rich({ text }: { text: string }) {
  return (
    <>
      {emphasis(text).map((part, i) =>
        part.strong ? (
          <Text key={i} span inherit fw={600} c="var(--mantine-color-text)">
            {part.text}
          </Text>
        ) : (
          part.text
        ),
      )}
    </>
  );
}

function Step({ step }: { step: GuideStep }) {
  return (
    <Stack gap={6}>
      <Text size="sm" c="dimmed" style={{ lineHeight: 1.6 }}>
        <Rich text={step.body} />
      </Text>
      {step.tip && (
        <Alert variant="light" color="brand" icon={<Lightbulb size={16} />} p="xs" radius="md">
          <Text size="sm">
            <Rich text={step.tip} />
          </Text>
        </Alert>
      )}
      {step.link && (
        <Button component={Link} to={step.link.to} variant="light" size="xs" w="fit-content" rightSection={<ArrowRight size={14} />}>
          {step.link.label}
        </Button>
      )}
    </Stack>
  );
}

function Chapter({
  chapter,
  number,
  done,
  onDoneChange,
}: {
  chapter: GuideChapter;
  number: number;
  done: boolean;
  onDoneChange: (done: boolean) => void;
}) {
  const headingId = `guide-${chapter.id}-title`;
  return (
    <Paper
      component="section"
      id={`guide-${chapter.id}`}
      aria-labelledby={headingId}
      withBorder
      radius="lg"
      p={{ base: 'md', sm: 'lg' }}
      bg="var(--app-surface-raised)"
      style={{ scrollMarginTop: SCROLL_MARGIN }}
    >
      <Stack gap="md">
        <Group gap="sm" wrap="nowrap" align="flex-start">
          <ThemeIcon size={32} radius="xl" variant={done ? 'filled' : 'light'} aria-hidden>
            {done ? (
              <CircleCheck size={18} />
            ) : (
              <Text fw={700} size="sm">
                {number}
              </Text>
            )}
          </ThemeIcon>
          <Stack gap={4} style={{ minWidth: 0 }}>
            <Title order={2} fz="h3" id={headingId}>
              {number}. {chapter.title}
            </Title>
            <Text size="sm" c="dimmed">
              {chapter.summary}
            </Text>
          </Stack>
        </Group>
        {chapter.when && (
          <Alert variant="default" icon={<Info size={16} />} p="xs" radius="md">
            <Text size="sm">{chapter.when}</Text>
          </Alert>
        )}
        <Timeline bulletSize={24} lineWidth={2} active={-1}>
          {chapter.steps.map((step, i) => (
            <Timeline.Item
              key={step.title}
              bullet={
                <Text size="xs" fw={600}>
                  {i + 1}
                </Text>
              }
              title={
                <Text fw={600} size="sm">
                  {step.title}
                </Text>
              }
            >
              <Step step={step} />
            </Timeline.Item>
          ))}
        </Timeline>
        <Box pt="sm" style={{ borderTop: '1px solid var(--app-border)' }}>
          <Checkbox
            label="Mark as done"
            description={done ? 'Nice work. Untick it to come back to this chapter.' : undefined}
            checked={done}
            onChange={(event) => onDoneChange(event.currentTarget.checked)}
            aria-label={`Mark “${chapter.title}” as done`}
          />
        </Box>
      </Stack>
    </Paper>
  );
}

function Contents({ chapters, done }: { chapters: GuideChapter[]; done: string[] }) {
  return (
    <Stack gap={2} component="nav" aria-label="Guide contents">
      <Text size="xs" fw={600} c="dimmed" tt="uppercase" px="sm" pb={4}>
        Contents
      </Text>
      {chapters.map((chapter, i) => {
        const finished = done.includes(chapter.id);
        return (
          <NavLink
            key={chapter.id}
            component="button"
            type="button"
            label={chapter.title}
            leftSection={
              finished ? (
                <CircleCheck size={16} color="var(--mantine-color-brand-filled)" aria-label="Done" />
              ) : (
                <Text size="xs" c="dimmed" w={16} ta="center">
                  {i + 1}
                </Text>
              )
            }
            onClick={() => scrollToSection(chapter.id)}
            style={{ borderRadius: 8 }}
          />
        );
      })}
      <NavLink
        component="button"
        type="button"
        label="Words you’ll meet"
        leftSection={<BookOpen size={16} />}
        onClick={() => scrollToSection(GLOSSARY_ID)}
        style={{ borderRadius: 8 }}
      />
    </Stack>
  );
}

function ProgressCard({ chapters, done }: { chapters: GuideChapter[]; done: string[] }) {
  const finished = chapters.filter((c) => done.includes(c.id)).length;
  const next = chapters.find((c) => !done.includes(c.id));
  return (
    <Paper withBorder radius="lg" p={{ base: 'md', sm: 'lg' }} bg="var(--app-surface-raised)">
      <Group justify="space-between" gap="md" align="center">
        <Stack gap={6} style={{ flex: '1 1 260px', minWidth: 0 }}>
          <Group gap="xs">
            <Text fw={600}>Your progress</Text>
            <Badge variant="light" size="sm">
              {finished} of {chapters.length} chapters done
            </Badge>
          </Group>
          <Progress value={(finished / chapters.length) * 100} aria-label="Chapters done" size="sm" radius="xl" />
          <Text size="sm" c="dimmed">
            Work through the chapters in order. Each step links to the screen it describes, so you can do it there and come back.
          </Text>
        </Stack>
        {next ? (
          <Button onClick={() => scrollToSection(next.id)} rightSection={<ArrowRight size={16} />}>
            {finished === 0 ? 'Start with chapter 1' : `Continue: ${next.title}`}
          </Button>
        ) : (
          <Button component={Link} to="/reports?report=profit_and_loss" variant="light" rightSection={<ArrowRight size={16} />}>
            Open your reports
          </Button>
        )}
      </Group>
    </Paper>
  );
}

function Glossary() {
  return (
    <Paper
      component="section"
      id={`guide-${GLOSSARY_ID}`}
      aria-labelledby="guide-glossary-title"
      withBorder
      radius="lg"
      p={{ base: 'md', sm: 'lg' }}
      bg="var(--app-surface-raised)"
      style={{ scrollMarginTop: SCROLL_MARGIN }}
    >
      <Stack gap="md">
        <Group gap="sm" wrap="nowrap">
          <ThemeIcon size={32} radius="xl" variant="light" aria-hidden>
            <BookOpen size={18} />
          </ThemeIcon>
          <Title order={2} fz="h3" id="guide-glossary-title">
            Words you’ll meet
          </Title>
        </Group>
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="md" component="dl" m={0}>
          {GLOSSARY.map((entry) => (
            <Box key={entry.term}>
              <Text component="dt" fw={600} size="sm">
                {entry.term}
              </Text>
              <Text component="dd" size="sm" c="dimmed" m={0}>
                {entry.meaning}
              </Text>
            </Box>
          ))}
        </SimpleGrid>
      </Stack>
    </Paper>
  );
}

/** A step-by-step tutorial of every feature, ending with the yearly report for the accountant. */
export function GuidePage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const chapters = GUIDE_CHAPTERS;
  const [done, setDone] = useLocalStorage<string[]>({ key: `guide-done:${business.id}`, defaultValue: [], getInitialValueInEffect: false });
  const markDone = (id: string, value: boolean) =>
    setDone((current) => (value ? [...current.filter((c) => c !== id), id] : current.filter((c) => c !== id)));

  const list = (
    <Stack gap="lg" style={{ minWidth: 0 }}>
      {chapters.map((chapter, i) => (
        <Chapter
          key={chapter.id}
          chapter={chapter}
          number={i + 1}
          done={done.includes(chapter.id)}
          onDoneChange={(value) => markDone(chapter.id, value)}
        />
      ))}
      <Glossary />
    </Stack>
  );

  return (
    <Page
      title="Guide"
      description={`Everything in ${BRAND.name}, step by step: from setting up your business to the yearly report you give your accountant. No bookkeeping knowledge needed.`}
      maxWidth={1180}
    >
      <ProgressCard chapters={chapters} done={done} />
      {isMobile ? (
        <Stack gap="lg">
          <Select
            label="Jump to a chapter"
            placeholder="Choose a chapter"
            data={[
              ...chapters.map((c, i) => ({ value: c.id, label: `${i + 1}. ${c.title}` })),
              { value: GLOSSARY_ID, label: 'Words you’ll meet' },
            ]}
            value={null}
            onChange={(value) => value && scrollToSection(value)}
          />
          {list}
        </Stack>
      ) : (
        // The columns stretch to the row's height, so the contents can stay in view (sticky) while the chapters scroll.
        <Grid gap="xl">
          <Grid.Col span={{ base: 12, md: 3 }}>
            <Box pos="sticky" top={SCROLL_MARGIN}>
              <Contents chapters={chapters} done={done} />
            </Box>
          </Grid.Col>
          <Grid.Col span={{ base: 12, md: 9 }}>{list}</Grid.Col>
        </Grid>
      )}
    </Page>
  );
}
