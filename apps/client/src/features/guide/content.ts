// The beginner's guide: every feature, in the order a new sole trader uses them, ending with the
// yearly report for their accountant. Labels in **double asterisks** are the app's own words (buttons,
// fields, tabs), so the guide reads the same as the screens. Links go to real routes; a test checks them.
import { BRAND } from '@/branding/brand';

export interface GuideLink {
  /** An app route, with a deep link where one exists (`/expenses?new=1` opens the form). */
  to: string;
  label: string;
}

export interface GuideStep {
  title: string;
  body: string;
  link?: GuideLink;
  tip?: string;
}

export interface GuideChapter {
  id: string;
  title: string;
  summary: string;
  /** When the chapter applies, if not always. */
  when?: string;
  steps: GuideStep[];
}

export const GUIDE_CHAPTERS: GuideChapter[] = [
  {
    id: 'start',
    title: 'Before you start',
    summary: `${BRAND.name} keeps your business records in one place and turns them into the reports your accountant needs at the end of the year. You don’t need to know bookkeeping: you record what happens, and the app does the double-entry for you.`,
    steps: [
      {
        title: 'Try the demo business first',
        body: 'Not sure yet? On the welcome screen, choose **Explore the demo** to open a made-up contractor’s books, with a year and a half of invoices, expenses, GST returns, mileage and reports. Try anything: it’s a separate business, labelled **Demo** on every screen, and it never sends email. The banner at the top has **Reset demo** to start it again, **Remove demo** to delete it, and **Set up my business** when you’re ready for your own.',
        link: { to: '/welcome', label: 'Open the welcome screen' },
        tip: 'Your own business is kept apart from the demo: switch between them from the business name at the top.',
      },
      {
        title: 'Gather your papers',
        body: 'Have these nearby: your business bank statements, receipts and bills, any invoices you’ve already sent, letters from IRD about GST or tax, and your accountant’s last annual report if you have one.',
      },
      {
        title: 'Pick the date your records start',
        body: 'Usually the first day of a financial year: 1 April for most sole traders in New Zealand. Everything from that date goes into the app. What happened before it comes in as opening balances (see “Bring in where you’re up to”).',
      },
      {
        title: 'Record things little and often',
        body: 'Add invoices, expenses and trips as they happen, or set aside half an hour each week. Then the yearly report takes minutes, not days.',
        tip: 'Tick **Mark as done** at the end of each chapter to keep track of how far you’ve got.',
      },
    ],
  },
  {
    id: 'business',
    title: 'Set up your business',
    summary:
      'Tell the app who you are and how you account for tax. It uses this on your invoices, for GST, and for the dates of your financial year.',
    steps: [
      {
        title: 'Check your business details',
        body: 'Open **Settings › Business details**. Check the **Business name**, **Your name or trading name** and **Business type**, then add your contact details and address: they’re printed on your invoices and quotes. You can add a logo here too.',
        link: { to: '/settings/business', label: 'Open business details' },
      },
      {
        title: 'Tell it about GST',
        body: 'Under **Tax and GST**, add your **NZBN** if you have one. If you’re registered for GST, tick **I’m registered for GST**, enter your **GST number** and the date you were **Registered from**, and choose your **GST accounting basis** (most small businesses use the payments basis). If you’re not registered, leave it unticked: the app then doesn’t charge or claim GST.',
        link: { to: '/settings/business', label: 'Open business details' },
        tip: 'Not registered yet? IRD requires you to register once your sales pass $60,000 in 12 months. The **Dashboard** tells you when your recorded sales go over it.',
      },
      {
        title: 'Set your financial year and reporting basis',
        body: 'Under **Financial year and reporting**, **Financial year starts** on 1 April for most sole traders. **Default reporting basis** decides how reports count money: **Accrual** counts income when you invoice and expenses when you incur them; **Cash** counts them when money moves. You can switch basis on any report later.',
        tip: 'Not sure which basis to use? Ask your accountant which one they prepare your accounts on, and pick the same.',
      },
      {
        title: 'Set up your invoices',
        body: 'In **Settings › Invoices & payments**, set your **Payment terms (days)**, your invoice and quote numbering, and your **Payment details** (account name and number), so customers know how to pay you.',
        link: { to: '/settings/invoicing', label: 'Open invoice settings' },
      },
    ],
  },
  {
    id: 'opening',
    title: 'Bring in where you’re up to',
    summary:
      'If your business was running before your records in the app start, copy the closing figures from your accountant’s last balance sheet. This year’s accounts then carry on from where theirs ended.',
    when: 'Skip this chapter if your business is brand new.',
    steps: [
      {
        title: 'Enter your opening balances',
        body: 'Go to **Year end › Opening balances**. Set **As at** to the last day before your records start, usually the end of the financial year your accountant last reported on (for example 31 March). Copy each figure from that balance sheet: **Business bank account**, **GST payable**, **Drawings to date**, **Funds introduced to date**, **Personal accounts**, **Withholding tax paid to date** and **Income tax paid to date**. Then choose **Save opening balances**.',
        link: { to: '/year-end?tab=opening', label: 'Open opening balances' },
        tip: 'Enter drawings and taxes paid as plain amounts: the app records them the way a balance sheet shows them. Retained earnings is the balancing figure, worked out for you.',
      },
      {
        title: 'Add the equipment you already own',
        body: 'Things that last several years, such as a computer or camera, are depreciated rather than claimed all at once. Go to **Year end › Assets** and choose **Add asset**. Give its name, **Date bought** and **Cost excluding GST**, tick **I had this before my opening balances**, and enter its **Value at the opening balance date** from your accountant’s depreciation schedule.',
        link: { to: '/year-end?tab=assets', label: 'Open assets' },
      },
      {
        title: 'Choose how each asset depreciates',
        body: 'Pick **Diminishing value** or **Straight line** and the **Rate (% a year)** from your accountant’s schedule or IRD’s rate finder. An asset costing $1,000 or less (excluding GST) can instead be deducted in full in the year you bought it: choose **Deduct in full**. If you also use it privately, enter the **Private use (%)**: that share is treated as drawings, not a business cost.',
      },
    ],
  },
  {
    id: 'customers',
    title: 'Add your customers',
    summary:
      'The people and businesses you work for. Each customer has a page with what they owe, what they’ve paid and how quickly they pay.',
    steps: [
      {
        title: 'Add a customer',
        body: 'Choose **Add customer** and enter their **Name**, **Email** (invoices and quotes are sent there) and **Billing address**. Add a **Business name** or **Contact person** if you need them on your invoices.',
        link: { to: '/customers?new=1', label: 'Add customer' },
      },
      {
        title: 'Check how a customer is doing',
        body: 'Open a customer from **Customers** to see **Outstanding**, **Overdue**, **Total invoiced**, **Total paid** and **Average time to pay**, with their invoices and quotes.',
        link: { to: '/customers', label: 'Open customers' },
      },
      {
        title: 'Send a customer their statement',
        body: '**Statement** on a customer opens their statement of account: what you invoiced, what they paid and what they owe. Once email is set up, choose **Email statement** from the **More actions** menu, pick the period and send it: the PDF is attached and the email shows the amount owing. Statements you’ve sent are listed under **Statements emailed**.',
        link: { to: '/customers', label: 'Open customers' },
      },
    ],
  },
  {
    id: 'quotes',
    title: 'Quote for work',
    summary: 'A quote tells a customer what the work will cost before you start. When they accept, it becomes an invoice in one step.',
    when: 'Optional: skip it if you don’t send quotes.',
    steps: [
      {
        title: 'Write a quote',
        body: 'Choose **New quote**, pick the **Customer**, and add a line for each item with its quantity and price. Turn on **Prices include GST** if the prices you type already include it. The totals are worked out for you.',
        link: { to: '/quotes/new', label: 'New quote' },
      },
      {
        title: 'Send it and record the answer',
        body: 'Download the PDF and send it to your customer. When they reply, open the quote and choose **Customer’s answer**, then **Accepted** or **Declined**.',
      },
      {
        title: 'Turn it into an invoice',
        body: 'On an accepted quote, choose **Create invoice**. The invoice starts as a draft with the same lines, ready to check and issue.',
        link: { to: '/quotes', label: 'Open quotes' },
      },
    ],
  },
  {
    id: 'invoices',
    title: 'Invoice and get paid',
    summary:
      'Invoices are how you ask to be paid. Issuing one records the sale in your books; recording payments keeps track of who still owes you.',
    steps: [
      {
        title: 'Create an invoice',
        body: 'Choose **New invoice**, pick the **Customer**, check the dates, and add a line for each item. Add a **Reference** such as a purchase order number if your customer asks for one. The totals and GST are worked out for you as you type.',
        link: { to: '/invoices/new', label: 'New invoice' },
      },
      {
        title: 'Issue it',
        body: 'A new invoice is a draft: you can change or delete it, and it isn’t in your books yet. When it’s right, choose **Issue invoice**. It gets its number and counts as income.',
      },
      {
        title: 'Send it to your customer',
        body: 'Download the PDF and email it yourself, then choose **Mark as sent** from the **More actions** menu. Once your email account is set up in **Settings › Email**, **Send** emails it for you, with the PDF attached.',
      },
      {
        title: 'Record each payment',
        body: 'When money arrives, open the invoice and choose **Record payment**. Enter the **Date received** and the **Amount**. If the customer deducted withholding tax (common for contractors paid schedular payments), enter it in **Tax withheld**: the amount is then the invoice total it settles, and the tax counts as tax you’ve paid.',
        tip: 'Use the **Unpaid** and **Overdue** tabs on **Invoices** to see who to chase.',
      },
      {
        title: 'Invoice someone regularly',
        body: 'For work you invoice every week or month, open an invoice you’ve sent and choose **More actions** › **Repeat this invoice**. Pick **How often** and the **Next invoice** date, and tick **Issue each invoice straight away** if you don’t want to check each one first. A new invoice with the same lines is made on each date when you open the app; see them all under **Recurring** on **Invoices**.',
        link: { to: '/invoices/recurring', label: 'Open recurring invoices' },
      },
      {
        title: 'Remind customers automatically',
        body: 'In **Settings › Invoices & payments**, under **Reminders**, turn on **Email reminders for unpaid invoices**, choose the **Days before the due date** and the **Days after the due date**, and choose **Save reminders**. When the app is open, each reminder is emailed once with the invoice attached, while the invoice is unpaid, and shows in the invoice’s **Emails**. To leave a customer out, turn off **Send invoice reminders** on their page.',
        link: { to: '/settings/invoicing', label: 'Open invoice settings' },
        tip: 'Reminders are sent through your email account, so set up Settings › Email first.',
      },
      {
        title: 'When a customer won’t pay',
        body: 'If you’ve given up on an invoice, open it, choose **More actions** › **Write off as a bad debt** and record the date. What’s left becomes a bad-debt expense and, on the invoice or hybrid GST basis, the GST in it comes off your GST return for that period. Keep a record of how you tried to collect it.',
      },
      {
        title: 'Fix a mistake',
        body: 'An issued invoice can still be edited from **More actions**. To withdraw it, choose **Cancel invoice** and give a reason: it stays in your records marked as cancelled, and its entries are reversed. Remove any payments first.',
        link: { to: '/invoices?tab=overdue', label: 'Open overdue invoices' },
      },
    ],
  },
  {
    id: 'expenses',
    title: 'Record what you spend',
    summary:
      'Every business cost you record lowers the profit you pay tax on, and you can claim back the GST if you’re registered. Keep the receipt with each one.',
    steps: [
      {
        title: 'Add an expense',
        body: 'Choose **Add expense**. Enter the **Date**, the **Amount paid** (the total on the receipt, including any GST), the **Supplier** and a **Category**. Under **GST**, choose **Includes GST (15%)**, **Zero-rated (0%)**, **Exempt from GST** or **No GST**.',
        link: { to: '/expenses?new=1', label: 'Add expense' },
        tip: 'Bought something secondhand from a person who isn’t GST-registered, such as on a marketplace? You can still claim 3/23 of the price: choose **Includes GST (15%)**, which works out the same amount, and note the seller’s name and address and the date.',
      },
      {
        title: 'Say how you paid',
        body: 'Under **Paid with**, choose **Business account**, **Credit card**, **Cash** or **Other**. If you paid with **My own money**, the business share is recorded as money you put into the business.',
      },
      {
        title: 'Say who it was for',
        body: 'Under **Who was it for?**, choose **Business**, **Partly business** (then enter the **Business share**, for example 50% of your phone), **Personal** (recorded as drawings, not a business cost) or **Recharge to a customer**.',
        tip: 'Business entertainment, such as taking a client to lunch, is usually only 50% deductible: keep it as **Business** and set the **Deductible share** to 50%. The app keeps the other half out of your taxable profit and adds IRD’s yearly GST adjustment to your GST return.',
      },
      {
        title: 'Attach the receipt',
        body: 'Open the expense from the list and drop a photo or PDF of the receipt into **Receipts**. Use the **Missing receipt** filter on **Expenses** to find the ones still without one.',
        link: { to: '/expenses', label: 'Open expenses' },
      },
      {
        title: 'Organise your categories',
        body: 'In **Settings › Expenses**, add categories that match how your accountant groups costs. For costs you can’t claim, such as IRD penalties, tick **Not deductible for tax**: reports then show them as an adjustment below net profit. If you sell goods or materials, tick **Cost of sales** on the categories for them: the profit and loss shows them before gross profit.',
        link: { to: '/settings/expenses', label: 'Open expense categories' },
      },
    ],
  },
  {
    id: 'income',
    title: 'Record other income',
    summary:
      'Money you receive without an invoice, such as cash sales, bank interest or a grant. Payments on invoices are recorded on the invoice instead.',
    steps: [
      {
        title: 'Record income',
        body: 'Choose **Record income**. Enter the **Date received**, the **Amount received** and the **Payer**. Choose the **Type** (**Sales** or **Other income**) and the **GST** that applies, and link a **Customer** if there is one.',
        link: { to: '/income?new=1', label: 'Record income' },
      },
    ],
  },
  {
    id: 'mileage',
    title: 'Track business mileage',
    summary:
      'If you use your own vehicle for work, you can claim the business kilometres. Each trip you record adds its claim to your expenses.',
    when: 'Skip it if you don’t drive for work.',
    steps: [
      {
        title: 'Add your vehicle and rate',
        body: 'On **Mileage**, choose **Add vehicle**, give it a **Name** such as “Work ute”, and set its **Default rate**: the rate per kilometre you claim, for example the kilometre rate IRD publishes for the year. The app never supplies rates for you.',
        link: { to: '/mileage', label: 'Open mileage' },
        tip: 'IRD’s kilometre rates have two tiers: enter the tier 1 rate as the **Rate per km**, then **Tier 1 limit** 14,000 and the **Tier 2 rate**. Once the vehicle’s recorded travel in the year passes 14,000 km, the business kilometres after that use the tier 2 rate. Only trips you record count, so record private trips too (with a **Business share** of 0%).',
      },
      {
        title: 'Record each trip',
        body: 'Choose **Record trip**. Pick the **Vehicle** and **Date**, enter the **Purpose**, **From** and **To**, then the distance in **Kilometres** or as odometer readings. For a trip there and back, enter the distance one way and tick **Return trip (there and back)**. Set the **Business share** if part of the trip was private. A route you’ve recorded twice in the last year appears under **Frequent trips**: choose it to fill in the trip, then check the date.',
        link: { to: '/mileage?new=1', label: 'Record trip' },
        tip: 'Claiming vehicle running costs as expenses instead? Without a logbook, IRD limits the business share to 25%; a 90-day logbook sets it for up to three years.',
      },
      {
        title: 'Update rates each year',
        body: 'In **Settings › Mileage**, add a new version of a rate when it changes, with the date it **Applies from** and, for IRD’s rates, the **Tier 1 limit** and **Tier 2 rate**. Trips you’ve already recorded keep the rate they used.',
        link: { to: '/settings/mileage', label: 'Open mileage settings' },
      },
      {
        title: 'See your totals',
        body: 'The **Report** tab on **Mileage** shows trips, kilometres and your claim by month and vehicle. **Export CSV** gives you a logbook file.',
        link: { to: '/mileage?tab=report', label: 'Open the mileage report' },
      },
    ],
  },
  {
    id: 'money',
    title: 'Record money in and out',
    summary:
      'Money that moves between you and the business, or between the business and IRD, isn’t income or an expense, but your balance sheet needs it to match your bank.',
    steps: [
      {
        title: 'Record money in or out',
        body: 'Go to **Year end › Money in and out** and choose **Record money in or out**. Pick the **Type**, then the **Date** and **Amount**:',
        link: { to: '/year-end?tab=money', label: 'Open money in and out' },
      },
      {
        title: 'Drawings and funds introduced',
        body: 'A **Drawing** is money you took out of the business for yourself, such as a transfer to your personal account. **Funds introduced** is your own money you put into the business.',
      },
      {
        title: 'GST and income tax',
        body: 'Record a **GST payment** when you pay IRD for a GST return and a **GST refund** when IRD pays you. Do the same with **Income tax payment** (including provisional tax) and **Income tax refund**.',
        tip: 'Tax a customer withheld from a payment isn’t recorded here: it goes in **Tax withheld** when you record the invoice payment.',
      },
      {
        title: 'Your GST returns',
        body: 'The **GST** page lists each GST period with its due date, the GST you collected and can claim, and **To pay IRD now**. Set how often you file under **Settings › Business details › GST returns** (for example every two months, with a period ending in March). **Return figures** on a return gives boxes 5 to 15 of IRD’s GST return, ready to copy into myIR. When you pay IRD, choose **Record payment** on that return: payments settle the oldest return first.',
        link: { to: '/gst', label: 'Open GST returns' },
        tip: 'The figures follow your GST accounting basis. Check them against IRD’s return form before you file.',
      },
      {
        title: 'Your income tax',
        body: 'The **Income tax** page estimates each year’s tax from your records: **Taxable income**, **Income tax**, the **Tax withheld by customers** and the **Residual income tax** left to pay. When that’s over $5,000, it shows provisional tax by IRD’s standard option and when each instalment is due, then the terminal tax for the year. When you pay one, choose **Record payment** next to it. Below it, **ACC levies** estimates what ACC will invoice for the year: enter the **Work levy rate** from your ACC invoice, turn on **I work full-time** if you work over 30 hours a week, and choose **Save**.',
        link: { to: '/income-tax', label: 'Open income tax' },
        tip: 'It’s an estimate from what you’ve recorded: income from outside the business isn’t included. Your accountant’s return and ACC’s invoice are the final figures.',
      },
    ],
  },
  {
    id: 'planning',
    title: 'Keep an eye on things',
    summary: 'Between invoices and receipts, a few screens help you see where you stand and plan ahead.',
    steps: [
      {
        title: 'Your dashboard',
        body: 'The **Dashboard** shows income, **Expenses**, **Net profit**, **Outstanding** and **Overdue** for the period, with quick buttons to add an invoice, expense, income, mileage or a customer. Switch between **Accrual** and **Cash** at the top. Under the charts, **Key dates** shows your next GST returns, provisional and terminal tax, your income tax return and the end of your financial year, with how many days are left. Open one to deal with it.',
        link: { to: '/dashboard', label: 'Open the dashboard' },
      },
      {
        title: 'Set money aside with allocations',
        body: `On **Allocations**, add rules that set aside part of each payment you receive, for example 20% for income tax. When you record money received, ${BRAND.name} proposes the split under **To review** and you choose **Confirm allocation** or **Skip**. Payments from before you had rules can be added with **Allocate earlier payments**. When you use money you set aside, such as paying IRD from your income tax savings, choose **Record spending** on the **Spending** tab so the balance stays true. It’s a plan: the app never moves money.`,
        link: { to: '/allocations', label: 'Open allocations' },
      },
      {
        title: 'Read your insights',
        body: '**Insights** explains what your figures mean, worked out from your records with simple rules. Estimates are marked, and nothing is shown without enough data behind it.',
        link: { to: '/insights', label: 'Open insights' },
      },
      {
        title: 'See every change',
        body: '**Settings › Activity log** lists every change to the business, with who made it and when.',
        link: { to: '/settings/activity', label: 'Open the activity log' },
      },
    ],
  },
  {
    id: 'year-end',
    title: 'Make your yearly report',
    summary:
      'At the end of the financial year, check your records are complete, post depreciation, and export the reports your accountant uses to prepare your annual accounts and tax return.',
    steps: [
      {
        title: 'Check the year is complete',
        body: 'Make sure every invoice for the year is issued and its payments recorded, every expense is in with its receipt, your trips are recorded, and drawings, funds introduced and tax payments are in **Money in and out**. **Key dates** on the **Dashboard** shows when the year ends.',
        link: { to: '/expenses', label: 'Check your expenses' },
      },
      {
        title: 'Post depreciation',
        body: 'Go to **Year end › Assets**. Under **Depreciation**, pick the **Financial year ending** and choose **Post depreciation**. If you change an asset or add one later, choose **Post again**.',
        link: { to: '/year-end?tab=assets', label: 'Open assets' },
      },
      {
        title: 'Review assets over $10,000 for GST',
        body: 'An asset costing over $10,000 (excluding GST), such as a car you also use privately, is reviewed at each balance date for two to ten years, depending on its cost. On **Year end › Assets**, under **GST change of use**, enter its **Business use** for the year just ended and choose **Save**. If its use since you bought it has moved 10 percentage points or more from what you claimed, or the GST difference is over $1,000, the app claims or pays back the difference in the GST return that includes the balance date.',
        link: { to: '/year-end?tab=assets', label: 'Open assets' },
        tip: 'Keep a logbook or diary of how the asset is used: it’s your evidence for the percentage.',
      },
      {
        title: 'Record assets you sold or scrapped',
        body: 'Open the asset on **Year end › Assets**, choose **Sell or scrap**, and enter the **Date sold or scrapped** and the **Amount received** (0 if you threw it away), then **Record sale**. There’s no depreciation in the year you sell: the sale is compared with the asset’s value at the start of that year. Getting more back is depreciation recovered, which is taxable; getting less is a loss you can claim. If you’re registered, the GST on the sale goes in that period’s return.',
        link: { to: '/year-end?tab=assets', label: 'Open assets' },
        tip: 'Recorded it by mistake? Open the asset and choose **Undo sale**.',
      },
      {
        title: 'Claim your home office',
        body: 'If you work from home, go to **Year end › Home office**, pick the year, and enter the **Business area (m²)**, the **Whole home (m²)** and the year’s **Mortgage interest, rates and rent**. Choose **Claim home office**: the app uses IRD’s square metre rate for that year and adds the claim to your expenses.',
        link: { to: '/year-end?tab=home-office', label: 'Open home office' },
        tip: 'With the square metre rate, don’t also record your home’s power, gas or internet as expenses: the rate covers them.',
      },
      {
        title: 'Check the profit and loss',
        body: 'Open **Reports** and choose **Profit and loss**. Set the period to **Last financial year** (or **This financial year** if it has just ended today) and the basis your accountant uses. It shows your income, expenses, non-deductible adjustments and net profit, beside the year before.',
        link: { to: '/reports?report=profit_and_loss', label: 'Open profit and loss' },
      },
      {
        title: 'Check the balance sheet',
        body: 'Choose **Balance sheet**. Its **Business bank account** figure should match your bank statement on the last day of the year. If it doesn’t, something is missing: often a drawing, a GST payment or a bank fee.',
        link: { to: '/reports?report=balance_sheet', label: 'Open the balance sheet' },
      },
      {
        title: 'Look at the rest of the annual reports',
        body: 'The **Statement of changes in equity** shows how your equity moved (profit, drawings, funds introduced, tax paid). The **Depreciation schedule** lists each asset’s value. The **GST summary** shows GST collected and claimable for any period, which helps with your GST returns.',
        link: { to: '/reports?report=equity_changes', label: 'Open changes in equity' },
      },
      {
        title: 'Export them for your accountant',
        body: 'On each report, choose **Export**, then **PDF**, **Excel (.xlsx)** or **CSV**. **All reports as PDF** (or Excel, or CSV) puts every report for the period in one file. Send your accountant the profit and loss, balance sheet, statement of changes in equity and depreciation schedule, and the **Expense summary** and **Income summary** if they want the detail. Or choose **Accountant pack for last year (.zip)**: every report as PDF and Excel, the general journal, the mileage log and your receipts in one file. If your accountant uses Xero, **Xero sales invoices (CSV)** and **Xero bills from expenses (CSV)** give the period in the files Xero imports (choose tax-exclusive amounts when importing).',
        link: { to: '/reports?report=depreciation_schedule', label: 'Open the depreciation schedule' },
        tip: 'Then start the new year straight away: keep recording as you go, and next year’s report is just as quick.',
      },
    ],
  },
  {
    id: 'account',
    title: 'You and your data',
    summary: 'Choose how the app looks, and keep a copy of your records.',
    steps: [
      {
        title: 'Your name',
        body: `There’s no account to sign in to: ${BRAND.name} opens straight into your books. In **Settings › Profile** you can add your name, which the activity log shows next to your changes.`,
        link: { to: '/settings/profile', label: 'Open profile' },
      },
      {
        title: 'Appearance and formats',
        body: '**Settings › Preferences** sets light or dark mode and how dates and numbers are shown. To make everything bigger or smaller, press Ctrl and + or Ctrl and − (Ctrl and 0 goes back to normal), as in a web browser.',
        link: { to: '/settings/preferences', label: 'Open preferences' },
      },
      {
        title: 'Back up your books',
        body: 'Your books are kept only on this computer. In **Settings › Your data**, choose **Back up now** to save one file with every business and its receipts, and keep it somewhere else, such as a USB drive or a cloud folder. To get your books back, on this computer or a new one, choose **Restore from a backup** and pick the file: your books are replaced by the backup’s, and Solo restarts. Solo reminds you on the dashboard when your last backup is over 30 days old.',
        link: { to: '/settings/data', label: 'Open your data' },
        tip: '**Download data** saves your records as a JSON file you can read or take to other software. Only a backup can be restored.',
      },
      {
        title: 'Send email from the app',
        body: 'To send invoices, quotes and statements straight from the app, open **Settings › Email**. Choose your provider under **Fill in for** (or enter the **Mail server (SMTP)**, **Port** and **Security**), then your **User name**, **Password** and the address to **Send from (email address)**, and choose **Save email settings**. Then **Send test email** to check it works.',
        link: { to: '/settings/email', label: 'Open email settings' },
        tip: 'Gmail, Outlook and iCloud need an app password (made in your account’s security settings), not your normal password.',
      },
      {
        title: 'Get your notices by email',
        body: 'The dashboard and **Insights** show notices, such as invoices that are overdue. To get them by email once a week, open **Settings › Email**, turn on **Email me my notices each week** under **Weekly notices email**, and choose **Save**. It lists the notices you haven’t dismissed, for every business. **Send now** sends one straight away.',
        link: { to: '/settings/email', label: 'Open email settings' },
      },
      {
        title: 'Check your business is set up properly',
        body: '**Settings › Business health** checks this business is ready to issue correct invoices, keep the records IRD expects and get paid: your GST number and registration date, your address and contact details, how customers pay you, email for invoices and reminders, customers who owe you without an email address, and expenses without a receipt. Problems come first, under **Fix now**, **Worth fixing** and **Suggestions**, each saying why it matters with a button to the screen that fixes it. Choose **Check again** after fixing something. Anything to fix now also shows as a notice on the dashboard until it’s fixed.',
        link: { to: '/settings/health', label: 'Open business health' },
        tip: 'Checks that don’t apply to you, such as the GST number when you aren’t registered for GST, are listed under **Doesn’t apply**.',
      },
      {
        title: 'Check everything works',
        body: '**Settings › System status** checks each part of the app on your computer (your books, receipts, PDF documents, email and updates) and says what to do about anything that isn’t working. Choose **Check again** after changing a setting.',
        link: { to: '/settings/status', label: 'Open system status' },
      },
      {
        title: 'Keep the app up to date',
        body: `When your copy of ${BRAND.name} can update itself, **Settings › Updates** shows the version you have. Choose **Check for updates**, then **Update and restart** when a new version is ready. Your records stay as they are.`,
        link: { to: '/settings/updates', label: 'Open updates' },
      },
      {
        title: 'More than one business',
        body: '**Settings › Your businesses** lists every business you keep books for. Each keeps its own customers, records and settings; switch between them from the top bar.',
        link: { to: '/settings/businesses', label: 'Open your businesses' },
      },
      {
        title: `Who makes ${BRAND.name}, and your privacy`,
        body: `**Settings › About** says who makes ${BRAND.name} and what it’s for, and links to its code on GitHub. It sends no analytics, and your books stay on your own computer.`,
        link: { to: '/settings/about', label: 'Open about' },
      },
      {
        title: 'What the licence allows',
        body: `**Settings › Licence** explains in a table what ${BRAND.name}’s open-source licence lets you do, with a link to the full text.`,
        link: { to: '/settings/licence', label: 'Open the licence' },
      },
    ],
  },
];

export interface GlossaryEntry {
  term: string;
  meaning: string;
}

/** Words the guide and the reports use, in plain English. */
export const GLOSSARY: GlossaryEntry[] = [
  {
    term: 'Accrual and cash basis',
    meaning:
      'Two ways to count money. Accrual counts income when you invoice and costs when you incur them; cash counts them when money actually moves.',
  },
  {
    term: 'Balance sheet',
    meaning: 'What the business owns (bank, equipment) and owes (such as GST to IRD) on one day, and what’s left for you: your equity.',
  },
  {
    term: 'Depreciation',
    meaning: 'Spreading the cost of equipment that lasts several years over those years, instead of claiming it all at once.',
  },
  {
    term: 'Drawings',
    meaning: 'Money you take out of the business for yourself. It isn’t a business cost, so it doesn’t lower your profit.',
  },
  { term: 'Equity', meaning: 'Your share of the business: everything it owns minus everything it owes.' },
  {
    term: 'Financial year',
    meaning: 'The 12 months your accounts and tax return cover: 1 April to 31 March for most sole traders in New Zealand.',
  },
  { term: 'Funds introduced', meaning: 'Your own money you put into the business, including business costs you paid personally.' },
  {
    term: 'GST',
    meaning:
      'Goods and services tax (15%). If you’re registered, you add it to your prices, claim back the GST on your costs, and pay IRD the difference.',
  },
  { term: 'Non-deductible', meaning: 'A cost you can’t claim against tax, such as an IRD penalty. It still reduces your bank balance.' },
  {
    term: 'Opening balances',
    meaning: 'The figures your records start from: your accountant’s last balance sheet, if the business was running before.',
  },
  { term: 'Profit and loss', meaning: 'Income minus expenses for a period. Your net profit is what income tax is worked out on.' },
  {
    term: 'Withholding tax',
    meaning:
      'Tax a customer deducts from what they pay you and pays to IRD for you (schedular payments). It counts towards your income tax.',
  },
];

export type TextPart = { text: string; strong: boolean };

/** Splits `**label**` markers into parts, so the app's own words can be shown in bold. */
export function emphasis(text: string): TextPart[] {
  return text
    .split(/(\*\*[^*]+\*\*)/)
    .filter(Boolean)
    .map((part) =>
      part.startsWith('**') && part.endsWith('**') ? { text: part.slice(2, -2), strong: true } : { text: part, strong: false },
    );
}
