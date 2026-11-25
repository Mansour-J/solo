-- Solo — shared read models. Views use security_invoker so row-level security applies to the caller
-- (a plain view would run with its owner's rights and bypass tenant isolation).

-- One row per invoice with the amount paid so far (active payments only). Every balance, overdue and
-- outstanding figure in the API derives from this single definition.
CREATE VIEW invoice_balances WITH (security_invoker = true) AS
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
       coalesce((SELECT sum(p.amount) FROM invoice_payments p WHERE p.invoice_id = i.id AND p.voided_at IS NULL), 0)::bigint AS paid
FROM invoices i;
GRANT SELECT ON invoice_balances TO solo_app;
