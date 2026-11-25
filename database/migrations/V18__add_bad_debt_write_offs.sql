-- Solo — writing off a bad debt (IR375, "Bad debts written off"). A write-off is recorded against the
-- invoice like a payment (method `write_off`): it settles what's left, posts a bad-debt expense and, on
-- the invoice or hybrid basis, a GST credit adjustment of 3/23 of the amount in that period. On the
-- payments basis nothing is posted: the sale was never counted.

CREATE OR REPLACE FUNCTION seed_chart_of_accounts(b uuid) RETURNS void
  LANGUAGE sql
AS $$
  INSERT INTO financial_accounts (business_id, code, name, account_type, system_key) VALUES
    (b, '1000', 'Business bank account', 'asset', 'bank'),
    (b, '1100', 'Accounts receivable', 'asset', 'accounts_receivable'),
    (b, '2100', 'GST', 'liability', 'gst'),
    (b, '3000', 'Owner funds introduced', 'equity', 'owner_funds'),
    (b, '3100', 'Owner drawings', 'equity', 'owner_drawings'),
    (b, '4000', 'Sales', 'income', 'sales'),
    (b, '4100', 'Other income', 'income', 'other_income')
  ON CONFLICT (business_id, system_key) DO NOTHING;

  INSERT INTO financial_accounts (business_id, code, name, account_type, system_key)
  SELECT b, first_free_account_code(b, a.lo, a.hi), a.name, a.account_type, a.system_key
  FROM (VALUES
    (6000, 6099, 'Mileage claims', 'expense', 'mileage'),
    (1500, 1599, 'Property, plant and equipment', 'asset', 'fixed_assets'),
    (3200, 3299, 'Withholding tax paid', 'equity', 'withholding_tax'),
    (3300, 3399, 'Income tax paid', 'equity', 'income_tax'),
    (3400, 3499, 'Personal accounts', 'equity', 'personal_accounts'),
    (3900, 3999, 'Retained earnings', 'equity', 'retained_earnings'),
    (6100, 6199, 'Depreciation', 'expense', 'depreciation'),
    (6200, 6299, 'Bad debts', 'expense', 'bad_debts')
  ) AS a (lo, hi, name, account_type, system_key)
  WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.system_key = a.system_key)
$$;

SELECT seed_chart_of_accounts(id) FROM businesses;

ALTER TABLE invoice_payments DROP CONSTRAINT invoice_payments_method_check;
ALTER TABLE invoice_payments ADD CONSTRAINT invoice_payments_method_check
  CHECK (method IN ('bank_transfer', 'cash', 'card', 'cheque', 'online', 'other', 'write_off'));
-- Nothing is withheld from money never received.
ALTER TABLE invoice_payments ADD CONSTRAINT invoice_payments_write_off_withholding_check
  CHECK (method <> 'write_off' OR withholding_tax = 0);

-- The amount written off, so an invoice settled by a write-off reads "Written off", not "Paid".
CREATE OR REPLACE VIEW invoice_balances WITH (security_invoker = true) AS
SELECT i.id AS invoice_id,
       i.business_id,
       i.customer_id,
       i.number,
       i.status,
       i.issue_date,
       i.due_date,
       i.sent_at,
       i.currency,
       i.total,
       coalesce((SELECT sum(p.amount) FROM invoice_payments p WHERE p.invoice_id = i.id AND p.voided_at IS NULL), 0)::bigint AS paid,
       coalesce((SELECT sum(p.amount) FROM invoice_payments p
                 WHERE p.invoice_id = i.id AND p.voided_at IS NULL AND p.method = 'write_off'), 0)::bigint AS written_off
FROM invoices i;
