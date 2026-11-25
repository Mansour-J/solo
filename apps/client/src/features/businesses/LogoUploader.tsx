import { Box, Button, Group, Image, Paper, Stack, Text } from '@mantine/core';
import { Dropzone, MIME_TYPES } from '@mantine/dropzone';
import { notifications } from '@mantine/notifications';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ImagePlus, Trash2 } from 'lucide-react';
import { useEffect, useMemo } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { api, fetchBlob, unwrap, uploadFile } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { businessKeys } from './api';

/** The business logo as an object URL (fetched with authentication). */
function useLogoUrl(businessId: string, hasLogo: boolean) {
  const { data: blob } = useQuery({
    queryKey: ['business', businessId, 'logo'],
    queryFn: () => fetchBlob(`/api/v1/businesses/${businessId}/logo`),
    enabled: hasLogo,
    staleTime: Infinity,
  });
  const url = useMemo(() => (blob && hasLogo ? URL.createObjectURL(blob) : null), [blob, hasLogo]);
  useEffect(() => () => (url ? URL.revokeObjectURL(url) : undefined), [url]);
  return url;
}

export function LogoUploader({ businessId, hasLogo, disabled }: { businessId: string; hasLogo: boolean; disabled?: boolean }) {
  const queryClient = useQueryClient();
  const url = useLogoUrl(businessId, hasLogo);
  const refresh = async () => {
    queryClient.removeQueries({ queryKey: ['business', businessId, 'logo'] });
    await queryClient.invalidateQueries({ queryKey: businessKeys.detail(businessId) });
    await queryClient.invalidateQueries({ queryKey: businessKeys.all });
    await queryClient.invalidateQueries({ queryKey: businessKeys.activityAll(businessId) });
  };
  const upload = useApiMutation((file: File) => uploadFile(`/api/v1/businesses/${businessId}/logo`, file), {
    successMessage: 'Logo updated',
    onSuccess: refresh,
  });
  const remove = useApiMutation(
    () => unwrap(api.DELETE('/api/v1/businesses/{business_id}/logo', { params: { path: { business_id: businessId } } })),
    { successMessage: 'Logo removed', onSuccess: refresh },
  );

  return (
    <Group align="flex-start" gap="lg" wrap="wrap">
      <Paper withBorder radius="md" w={120} h={120} style={{ display: 'grid', placeItems: 'center', overflow: 'hidden' }}>
        {url ? (
          <Image src={url} alt="Business logo" fit="contain" w="100%" h="100%" p="xs" />
        ) : (
          <Text size="xs" c="dimmed" ta="center" px="xs">
            No logo yet
          </Text>
        )}
      </Paper>
      <Stack gap="xs" style={{ flex: '1 1 240px' }}>
        <Dropzone
          onDrop={(files) => files[0] && upload.mutate(files[0])}
          onReject={() => notifications.show({ color: 'red', message: 'Use a PNG, JPEG or WebP image of up to 2 MB.' })}
          accept={[MIME_TYPES.png, MIME_TYPES.jpeg, MIME_TYPES.webp]}
          maxSize={2 * 1024 * 1024}
          maxFiles={1}
          loading={upload.isPending}
          disabled={disabled}
          radius="md"
        >
          <Group justify="center" gap="sm" mih={72} style={{ pointerEvents: 'none' }}>
            <ImagePlus size={22} strokeWidth={1.75} />
            <Box>
              <Text size="sm" fw={500}>
                Drop your logo here or click to choose
              </Text>
              <Text size="xs" c="dimmed">
                PNG, JPEG or WebP, up to 2 MB. Shown on your invoices and quotes.
              </Text>
            </Box>
          </Group>
        </Dropzone>
        {hasLogo && (
          <Group>
            <Button
              variant="subtle"
              color="red"
              size="xs"
              leftSection={<Trash2 size={14} />}
              loading={remove.isPending}
              disabled={disabled}
              onClick={async () => {
                if (
                  await confirmAction({
                    title: 'Remove logo?',
                    message: 'Your documents will show your business name instead.',
                    confirmLabel: 'Remove',
                    danger: true,
                  })
                ) {
                  remove.mutate(undefined);
                }
              }}
            >
              Remove logo
            </Button>
          </Group>
        )}
      </Stack>
    </Group>
  );
}
