import { Alert, Anchor, Button, Group } from '@mantine/core';
import { useForm } from '@mantine/form';
import { ArrowLeft } from 'lucide-react';
import { Link, useNavigate, useParams, useSearchParams } from 'react-router';
import { Page } from '@/components/Page';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { financialKeys, useInvoice, useQuote, type Invoice, type Quote } from './api';
import { DocumentEditor } from './DocumentEditor';
import { documentSchema, fromDocument, newDocument, toLines, type DocumentValues } from './documentForm';

type Kind = 'invoice' | 'quote';

function EditorForm({ kind, initial, existing }: { kind: Kind; initial: DocumentValues; existing?: Invoice | Quote }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const form = useForm<DocumentValues>({ initialValues: initial, validate: zodValidate<DocumentValues>(documentSchema) });
  const base = kind === 'invoice' ? '/invoices' : '/quotes';
  const path = { business_id: business.id };
  const body = (v: DocumentValues) => ({
    customer_id: v.customer_id ?? '',
    issue_date: v.issue_date,
    reference: v.reference,
    prices_include_gst: v.prices_include_gst,
    notes: v.notes,
    footer: v.footer,
    lines: toLines(v.lines),
  });

  const save = useApiMutation(
    async ({ values, issue }: { values: DocumentValues; issue: boolean }): Promise<string> => {
      if (kind === 'invoice') {
        const payload = { ...body(values), due_date: values.end_date };
        const saved = existing
          ? await unwrap(
              api.PUT('/api/v1/businesses/{business_id}/invoices/{invoice_id}', {
                params: { path: { ...path, invoice_id: existing.id } },
                body: payload,
              }),
            )
          : await unwrap(api.POST('/api/v1/businesses/{business_id}/invoices', { params: { path }, body: payload }));
        if (issue)
          await unwrap(
            api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/issue', {
              params: { path: { ...path, invoice_id: saved.id } },
            }),
          );
        return saved.id;
      }
      const payload = { ...body(values), expiry_date: values.end_date };
      const saved = existing
        ? await unwrap(
            api.PUT('/api/v1/businesses/{business_id}/quotes/{quote_id}', {
              params: { path: { ...path, quote_id: existing.id } },
              body: payload,
            }),
          )
        : await unwrap(api.POST('/api/v1/businesses/{business_id}/quotes', { params: { path }, body: payload }));
      return saved.id;
    },
    {
      form,
      invalidate: financialKeys(business.id),
      successMessage: (_, { issue }) =>
        issue ? 'Invoice issued' : existing ? 'Changes saved' : kind === 'invoice' ? 'Draft saved' : 'Quote saved',
      onSuccess: (id) => navigate(`${base}/${id}`),
    },
  );

  const submit = (issue: boolean) => form.onSubmit((values) => save.mutate({ values, issue }));
  const issued = existing && 'status' in existing && existing.status === 'issued';
  return (
    <form onSubmit={submit(false)} noValidate>
      <Page
        eyebrow={
          <Anchor component={Link} to={existing ? `${base}/${existing.id}` : base} size="sm" c="dimmed">
            <Group gap={4}>
              <ArrowLeft size={14} /> {existing ? 'Back' : kind === 'invoice' ? 'Invoices' : 'Quotes'}
            </Group>
          </Anchor>
        }
        title={existing ? `Edit ${kind} ${existing.number ?? 'draft'}` : kind === 'invoice' ? 'New invoice' : 'New quote'}
        actions={
          <Group gap="xs">
            <Button variant="default" component={Link} to={existing ? `${base}/${existing.id}` : base}>
              Cancel
            </Button>
            {kind === 'invoice' && !existing && (
              <Button variant="default" type="submit" loading={save.isPending && !save.variables?.issue}>
                Save draft
              </Button>
            )}
            {kind === 'invoice' && !existing ? (
              <Button onClick={() => submit(true)()} loading={save.isPending && save.variables?.issue}>
                Save and issue
              </Button>
            ) : (
              <Button type="submit" loading={save.isPending}>
                Save
              </Button>
            )}
          </Group>
        }
      >
        {issued && (
          <Alert color="blue" variant="light">
            This invoice is issued. Saving changes reverses its original entries in your books and records the corrected ones, so the
            history stays traceable.
          </Alert>
        )}
        <DocumentEditor
          kind={kind}
          form={form}
          businessId={business.id}
          currency={business.currency}
          gstRegistered={business.gst_registered}
          customerLocked={!!existing && 'paid' in existing && existing.paid > 0}
        />
      </Page>
    </form>
  );
}

function NewDocument({ kind }: { kind: Kind }) {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  const [params] = useSearchParams();
  if (settings.isPending) return <ListSkeleton rows={6} height={64} />;
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  const notes = kind === 'invoice' ? settings.data.default_invoice_notes : '';
  const footer = kind === 'invoice' ? settings.data.default_invoice_footer : '';
  return <EditorForm kind={kind} initial={newDocument(business.today, business.gst_registered, params.get('customer'), notes, footer)} />;
}

function EditDocument({ kind, id }: { kind: Kind; id: string }) {
  const business = useCurrentBusiness();
  const invoice = useInvoice(business.id, kind === 'invoice' ? id : undefined);
  const quote = useQuote(business.id, kind === 'quote' ? id : undefined);
  const query = kind === 'invoice' ? invoice : quote;
  if (query.isPending) return <ListSkeleton rows={6} height={64} />;
  if (query.isError) return <ErrorState error={query.error} onRetry={() => void query.refetch()} />;
  const doc = query.data;
  return <EditorForm key={doc.id} kind={kind} initial={fromDocument(doc)} existing={doc} />;
}

export function InvoiceEditorPage() {
  const { invoiceId } = useParams();
  return invoiceId ? <EditDocument kind="invoice" id={invoiceId} /> : <NewDocument kind="invoice" />;
}

export function QuoteEditorPage() {
  const { quoteId } = useParams();
  return quoteId ? <EditDocument kind="quote" id={quoteId} /> : <NewDocument kind="quote" />;
}
