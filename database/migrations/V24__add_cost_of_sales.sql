-- Solo — cost of sales: expense accounts for direct costs of what's sold (materials, stock,
-- subcontractors). The profit and loss shows them before gross profit (sales less cost of sales).

ALTER TABLE financial_accounts
  ADD COLUMN cost_of_sales boolean NOT NULL DEFAULT false,
  ADD CONSTRAINT financial_accounts_cost_of_sales_check CHECK (NOT cost_of_sales OR account_type = 'expense'),
  ADD CONSTRAINT financial_accounts_cost_of_sales_deductible_check CHECK (NOT (cost_of_sales AND non_deductible));
