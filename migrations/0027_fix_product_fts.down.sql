-- Revert to the original content-table FTS5 (with the known bug)
DROP TRIGGER IF EXISTS product_fts_delete;
DROP TRIGGER IF EXISTS product_fts_update;
DROP TRIGGER IF EXISTS product_fts_insert;
DROP TABLE IF EXISTS product_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS product_fts USING fts5(
    product_id,
    name,
    sku,
    description,
    content='product',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS product_fts_insert AFTER INSERT ON product BEGIN
    INSERT INTO product_fts(product_id, name, sku, description)
    VALUES (new.id, new.name, new.sku, new.description);
END;

CREATE TRIGGER IF NOT EXISTS product_fts_update AFTER UPDATE ON product BEGIN
    DELETE FROM product_fts WHERE product_id = old.id;
    INSERT INTO product_fts(product_id, name, sku, description)
    VALUES (new.id, new.name, new.sku, new.description);
END;

CREATE TRIGGER IF NOT EXISTS product_fts_delete AFTER DELETE ON product BEGIN
    DELETE FROM product_fts WHERE product_id = old.id;
END;
