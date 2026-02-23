use crate::domain::product::{Product, ProductId};

#[derive(Default)]
pub struct Catalog {
    products: Vec<Product>,
}

impl Catalog {
    pub fn new(products: Vec<Product>) -> Self {
        Self { products }
    }

    pub fn find(&self, product_id: &ProductId) -> Option<&Product> {
        self.products.iter().find(|product| &product.id == product_id)
    }
}
