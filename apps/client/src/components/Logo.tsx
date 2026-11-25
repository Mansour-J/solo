import { Group, Image, Text } from '@mantine/core';
import { BRAND, LOGO_MARK_URL } from '@/branding/brand';

export function LogoMark({ size = 28 }: { size?: number }) {
  return <Image src={LOGO_MARK_URL} alt="" w={size} h={size} radius="sm" aria-hidden />;
}

/** Logo mark with the product name, both from the brand configuration. */
export function Wordmark({ size = 28, inverted = false }: { size?: number; inverted?: boolean }) {
  return (
    <Group gap={10} wrap="nowrap" aria-label={BRAND.name}>
      <LogoMark size={size} />
      <Text fw={700} fz={size * 0.62} lh={1} c={inverted ? 'white' : undefined} style={{ letterSpacing: '-0.01em' }}>
        {BRAND.shortName}
      </Text>
    </Group>
  );
}
