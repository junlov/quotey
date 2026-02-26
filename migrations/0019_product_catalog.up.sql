-- Product catalog tables for quotey-101 (Product Catalog Structure Overhaul).
-- Introduces product families, enriched product rows, and per-product
-- attribute definitions that the constraint/pricing engines use.

CREATE TABLE IF NOT EXISTS product_family (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS product (
    id           TEXT PRIMARY KEY,
    sku          TEXT NOT NULL UNIQUE,
    name         TEXT NOT NULL,
    description  TEXT,
    product_type TEXT NOT NULL DEFAULT 'simple',  -- simple | configurable | bundle
    family_id    TEXT,
    base_price   TEXT,          -- DECIMAL stored as TEXT for precision
    currency     TEXT NOT NULL DEFAULT 'USD',
    active       INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    FOREIGN KEY (family_id) REFERENCES product_family(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_product_sku ON product(sku);
CREATE INDEX IF NOT EXISTS idx_product_family_id ON product(family_id);
CREATE INDEX IF NOT EXISTS idx_product_active ON product(active);
CREATE INDEX IF NOT EXISTS idx_product_type ON product(product_type);

-- Full-text search on product name/sku/description for catalog_search.
CREATE VIRTUAL TABLE IF NOT EXISTS product_fts USING fts5(
    product_id,
    name,
    sku,
    description,
    content='product',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS in sync with the product table.
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

-- Per-product attribute definitions (configurable dimensions).
CREATE TABLE IF NOT EXISTS product_attribute (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id    TEXT NOT NULL,
    key           TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    value_type    TEXT NOT NULL,   -- JSON: {"Integer":{"min":1,"max":1000}} etc.
    required      INTEGER NOT NULL DEFAULT 0,
    default_value TEXT,
    sort_order    INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (product_id) REFERENCES product(id) ON DELETE CASCADE,
    UNIQUE(product_id, key)
);

CREATE INDEX IF NOT EXISTS idx_product_attribute_product ON product_attribute(product_id);

-- Bundle membership (for bundle products).
CREATE TABLE IF NOT EXISTS product_bundle_member (
    bundle_id  TEXT NOT NULL,
    member_id  TEXT NOT NULL,
    quantity   INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bundle_id, member_id),
    FOREIGN KEY (bundle_id) REFERENCES product(id) ON DELETE CASCADE,
    FOREIGN KEY (member_id) REFERENCES product(id) ON DELETE CASCADE
);
