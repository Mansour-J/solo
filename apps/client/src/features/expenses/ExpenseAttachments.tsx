import {
  ActionIcon,
  Box,
  Group,
  Image,
  Modal,
  Paper,
  Skeleton,
  Stack,
  Text,
  ThemeIcon,
  Tooltip,
  UnstyledButton,
  type ImageProps,
} from '@mantine/core';
import { Dropzone } from '@mantine/dropzone';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Download, ExternalLink, FileText, Paperclip, Trash2 } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate } from '@/lib/format';
import { api, unwrap, uploadFile } from '@/services/api/client';
import { errorMessage } from '@/services/api/errors';
import { useApiMutation } from '@/services/api/query';
import { saveBlob } from '@/services/platform';
import { attachmentQuery, expenseChangeKeys, expenseKeys, type ExpenseAttachment, type ExpenseDetail } from './api';
import { formatFileSize, isImage, MAX_RECEIPT_BYTES, MAX_RECEIPTS, RECEIPT_TYPES, rejectionMessage } from './attachments';

/** An image from a blob: the object URL is created when shown and revoked when unmounted. */
function BlobImage({ blob, alt, ...props }: { blob: Blob; alt: string } & ImageProps) {
  const ref = useRef<HTMLImageElement>(null);
  useEffect(() => {
    const img = ref.current;
    if (!img) return;
    const url = URL.createObjectURL(blob);
    img.src = url;
    return () => {
      img.removeAttribute('src');
      URL.revokeObjectURL(url);
    };
  }, [blob]);
  return <Image ref={ref} alt={alt} {...props} />;
}

function showError(error: unknown) {
  notifications.show({ color: 'red', title: 'That didn’t work', message: errorMessage(error) });
}

function AttachmentRow({
  businessId,
  expenseId,
  attachment,
  canRemove,
  removing,
  onRemove,
}: {
  businessId: string;
  expenseId: string;
  attachment: ExpenseAttachment;
  canRemove: boolean;
  removing: boolean;
  onRemove: () => void;
}) {
  const queryClient = useQueryClient();
  const image = isImage(attachment.content_type);
  const file = useQuery({ ...attachmentQuery(businessId, expenseId, attachment.id), enabled: image });
  const [previewOpened, preview] = useDisclosure(false);
  const getBlob = () => queryClient.fetchQuery(attachmentQuery(businessId, expenseId, attachment.id));

  const download = async () => {
    try {
      saveBlob(await getBlob(), attachment.filename);
    } catch (error) {
      showError(error);
    }
  };

  // Opened synchronously so popup blockers allow it, then pointed at the file once it has loaded.
  const openPdf = async () => {
    const tab = window.open('about:blank', '_blank');
    try {
      const blob = new Blob([await getBlob()], { type: attachment.content_type });
      if (!tab) {
        saveBlob(blob, attachment.filename);
        return;
      }
      const url = URL.createObjectURL(blob);
      tab.opener = null;
      tab.location.href = url;
      setTimeout(() => URL.revokeObjectURL(url), 60_000);
    } catch (error) {
      tab?.close();
      showError(error);
    }
  };

  const thumbnail = image ? (
    <UnstyledButton onClick={preview.open} aria-label={`Preview ${attachment.filename}`}>
      <Paper withBorder radius="sm" w={48} h={48} style={{ overflow: 'hidden' }}>
        {file.data ? (
          <BlobImage blob={file.data} alt="" w={48} h={48} fit="cover" />
        ) : file.isError ? (
          <Box w={48} h={48} style={{ display: 'grid', placeItems: 'center' }}>
            <Paperclip size={18} />
          </Box>
        ) : (
          <Skeleton w={48} h={48} />
        )}
      </Paper>
    </UnstyledButton>
  ) : (
    <ThemeIcon size={48} radius="sm" variant="light" color="red">
      <FileText size={22} strokeWidth={1.75} />
    </ThemeIcon>
  );

  return (
    <Group gap="sm" wrap="nowrap" py={6} style={{ borderBottom: '1px solid var(--app-border)' }}>
      {thumbnail}
      <Box style={{ flex: 1, minWidth: 0 }}>
        <Text size="sm" fw={500} truncate="end" title={attachment.filename}>
          {attachment.filename}
        </Text>
        <Text size="xs" c="dimmed">
          {formatFileSize(attachment.size_bytes)} · added {formatDate(attachment.created_at)}
        </Text>
      </Box>
      <Group gap={2} wrap="nowrap">
        <Tooltip label="Open">
          <ActionIcon aria-label={`Open ${attachment.filename}`} onClick={image ? preview.open : () => void openPdf()}>
            <ExternalLink size={16} />
          </ActionIcon>
        </Tooltip>
        <Tooltip label="Download">
          <ActionIcon aria-label={`Download ${attachment.filename}`} onClick={() => void download()}>
            <Download size={16} />
          </ActionIcon>
        </Tooltip>
        {canRemove && (
          <Tooltip label="Remove">
            <ActionIcon aria-label={`Remove ${attachment.filename}`} color="red" loading={removing} onClick={onRemove}>
              <Trash2 size={16} />
            </ActionIcon>
          </Tooltip>
        )}
      </Group>
      {image && (
        <Modal opened={previewOpened} onClose={preview.close} title={attachment.filename} size="xl">
          {file.data ? (
            <BlobImage blob={file.data} alt={attachment.filename} fit="contain" mah="75vh" />
          ) : file.isError ? (
            <Text size="sm" c="dimmed">
              {errorMessage(file.error)}
            </Text>
          ) : (
            <Skeleton h={320} />
          )}
        </Modal>
      )}
    </Group>
  );
}

/** Receipts attached to an expense: upload, preview, download and remove. */
export function ExpenseAttachments({ expense }: { expense: ExpenseDetail }) {
  const business = useCurrentBusiness();
  const queryClient = useQueryClient();
  const remaining = MAX_RECEIPTS - expense.attachments.length;
  const editable = !expense.voided;
  const upload = useApiMutation(
    (file: File) =>
      uploadFile(`/api/v1/businesses/${business.id}/expenses/${expense.id}/attachments`, file, 'POST') as Promise<ExpenseDetail>,
    {
      successMessage: (_, file) => `${file.name} attached`,
      invalidate: expenseChangeKeys(business.id, { categories: false }),
    },
  );
  const remove = useApiMutation(
    (attachment: ExpenseAttachment) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/expenses/{expense_id}/attachments/{attachment_id}', {
          params: { path: { business_id: business.id, expense_id: expense.id, attachment_id: attachment.id } },
        }),
      ),
    {
      successMessage: (_, attachment) => `${attachment.filename} removed`,
      invalidate: expenseChangeKeys(business.id, { categories: false }),
      onSuccess: (_, attachment) => queryClient.removeQueries({ queryKey: expenseKeys.attachment(business.id, attachment.id) }),
    },
  );

  const uploadAll = async (files: File[]) => {
    for (const file of files) {
      try {
        await upload.mutateAsync(file);
      } catch {
        return; // The mutation has already shown the error.
      }
    }
  };

  const confirmRemove = async (attachment: ExpenseAttachment) => {
    const confirmed = await confirmAction({
      title: 'Remove this receipt?',
      message: `“${attachment.filename}” will be deleted. This can’t be undone.`,
      confirmLabel: 'Remove',
      danger: true,
    });
    if (confirmed) remove.mutate(attachment);
  };

  return (
    <Stack gap="xs">
      {expense.attachments.length === 0 && !editable && (
        <Text size="sm" c="dimmed">
          No receipts attached.
        </Text>
      )}
      {expense.attachments.map((attachment) => (
        <AttachmentRow
          key={attachment.id}
          businessId={business.id}
          expenseId={expense.id}
          attachment={attachment}
          canRemove={editable}
          removing={remove.isPending && remove.variables?.id === attachment.id}
          onRemove={() => void confirmRemove(attachment)}
        />
      ))}
      {editable &&
        (remaining > 0 ? (
          <Dropzone
            onDrop={(files) => void uploadAll(files)}
            onReject={(rejections) =>
              notifications.show({
                color: 'red',
                message: rejectionMessage(
                  rejections.flatMap((r) => r.errors.map((e) => e.code)),
                  remaining,
                ),
              })
            }
            accept={RECEIPT_TYPES}
            maxSize={MAX_RECEIPT_BYTES}
            maxFiles={remaining}
            multiple={remaining > 1}
            loading={upload.isPending}
            radius="md"
            mt={expense.attachments.length > 0 ? 'xs' : 0}
          >
            <Group justify="center" gap="sm" mih={64} style={{ pointerEvents: 'none' }}>
              <Paperclip size={20} strokeWidth={1.75} />
              <Box>
                <Text size="sm" fw={500}>
                  Drop a receipt here or click to choose
                </Text>
                <Text size="xs" c="dimmed">
                  PDF, JPEG, PNG or WebP, up to 10 MB each. Up to {MAX_RECEIPTS} per expense.
                </Text>
              </Box>
            </Group>
          </Dropzone>
        ) : (
          <Text size="xs" c="dimmed">
            This expense has the maximum of {MAX_RECEIPTS} attachments. Remove one to add another.
          </Text>
        ))}
    </Stack>
  );
}
