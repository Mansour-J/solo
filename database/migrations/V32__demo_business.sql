-- Solo — a demo business to explore the app with (SL-710).
--
-- On first run the owner can explore a business filled with made-up records instead of setting up
-- their own. It is an ordinary business (so its records are isolated like any other), marked here so
-- the app labels it everywhere, never sends email from it and can reset or remove it. A business is a
-- demo from the day it is created; the flag never changes.

ALTER TABLE businesses ADD COLUMN is_demo boolean NOT NULL DEFAULT false;
