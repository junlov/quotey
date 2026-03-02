-- Fix FTS5 product search: the product_fts virtual table used `product_id` as
-- a column name, but with `content='product'` FTS5 resolves columns by position
-- against the content table.  When SQLite internally reads from the content table
-- it looks for `product.product_id` which does not exist (the column is `id`).
--
-- Solution: drop and recreate without a content table.  The triggers already
-- populate the FTS table explicitly, so we use a standalone (non-content) FTS
-- table which avoids the column-name mismatch entirely.

-- Drop old triggers first
DROP TRIGGER IF EXISTS product_fts_delete;
DROP TRIGGER IF EXISTS product_fts_update;
DROP TRIGGER IF EXISTS product_fts_insert;

-- Drop old FTS table
DROP TABLE IF EXISTS product_fts;

-- Recreate as standalone FTS5 table (no content= directive)
CREATE VIRTUAL TABLE IF NOT EXISTS product_fts USING fts5(
    product_id,
    name,
    sku,
    description,
    tokenize='porter unicode61'
);

-- Re-populate from existing products
INSERT INTO product_fts(product_id, name, sku, description)
SELECT id, name, sku, COALESCE(description, '') FROM product;

-- Recreate triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS product_fts_insert AFTER INSERT ON product BEGIN
    INSERT INTO product_fts(product_id, name, sku, description)
    VALUES (new.id, new.name, new.sku, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS product_fts_update AFTER UPDATE ON product BEGIN
    DELETE FROM product_fts WHERE product_id = old.id;
    INSERT INTO product_fts(product_id, name, sku, description)
    VALUES (new.id, new.name, new.sku, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS product_fts_delete AFTER DELETE ON product BEGIN
    DELETE FROM product_fts WHERE product_id = old.id;
END;
