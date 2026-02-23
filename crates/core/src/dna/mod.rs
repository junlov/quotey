use serde_json::{json, Value};

use crate::domain::quote::{Quote, QuoteLine};

pub const FINGERPRINT_BITS: usize = 128;
pub const FINGERPRINT_BYTES: usize = FINGERPRINT_BITS / 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigurationFingerprint {
    pub hash_hex: String,
    pub hash_bytes: [u8; FINGERPRINT_BYTES],
    pub vector: [u8; FINGERPRINT_BITS],
}

#[derive(Clone, Debug, Default)]
pub struct FingerprintGenerator;

impl FingerprintGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_from_json(&self, configuration: &Value) -> ConfigurationFingerprint {
        let mut features = Vec::new();
        collect_features(configuration, "$", &mut features);
        if features.is_empty() {
            features.push("$=empty".to_owned());
        }

        let mut accumulator = [0i32; FINGERPRINT_BITS];
        for feature in features {
            let (left, right) = feature_hashes(&feature);
            for bit in 0..64 {
                if (left >> bit) & 1 == 1 {
                    accumulator[bit] += 1;
                } else {
                    accumulator[bit] -= 1;
                }

                if (right >> bit) & 1 == 1 {
                    accumulator[bit + 64] += 1;
                } else {
                    accumulator[bit + 64] -= 1;
                }
            }
        }

        let mut hash_bytes = [0u8; FINGERPRINT_BYTES];
        let mut vector = [0u8; FINGERPRINT_BITS];
        for bit in 0..FINGERPRINT_BITS {
            let is_one = accumulator[bit] >= 0;
            vector[bit] = u8::from(is_one);
            if is_one {
                hash_bytes[bit / 8] |= 1 << (7 - (bit % 8));
            }
        }

        ConfigurationFingerprint { hash_hex: bytes_to_hex(&hash_bytes), hash_bytes, vector }
    }

    pub fn generate_from_lines(&self, lines: &[QuoteLine]) -> ConfigurationFingerprint {
        self.generate_from_json(&configuration_from_lines(lines))
    }

    pub fn generate_from_quote(&self, quote: &Quote) -> ConfigurationFingerprint {
        self.generate_from_lines(&quote.lines)
    }

    pub fn hamming_distance(
        &self,
        left: &ConfigurationFingerprint,
        right: &ConfigurationFingerprint,
    ) -> usize {
        left.vector.iter().zip(right.vector.iter()).filter(|(a, b)| a != b).count()
    }

    pub fn similarity_score(
        &self,
        left: &ConfigurationFingerprint,
        right: &ConfigurationFingerprint,
    ) -> f32 {
        let distance = self.hamming_distance(left, right) as f32;
        1.0 - distance / FINGERPRINT_BITS as f32
    }
}

pub fn configuration_from_lines(lines: &[QuoteLine]) -> Value {
    let mut line_entries: Vec<(String, u32, String)> = lines
        .iter()
        .map(|line| {
            (line.product_id.0.clone(), line.quantity, line.unit_price.normalize().to_string())
        })
        .collect();
    line_entries.sort();

    json!({
        "lines": line_entries
            .into_iter()
            .map(|(product_id, quantity, unit_price)| {
                json!({
                    "product_id": product_id,
                    "quantity": quantity,
                    "unit_price": unit_price,
                })
            })
            .collect::<Vec<_>>()
    })
}

fn collect_features(value: &Value, path: &str, output: &mut Vec<String>) {
    match value {
        Value::Null => output.push(format!("{path}=null")),
        Value::Bool(v) => output.push(format!("{path}=bool:{v}")),
        Value::Number(v) => output.push(format!("{path}=number:{v}")),
        Value::String(v) => output.push(format!("{path}=string:{v}")),
        Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                collect_features(item, &format!("{path}[{index}]"), output);
            }
        }
        Value::Object(values) => {
            let mut keys: Vec<&str> = values.keys().map(String::as_str).collect();
            keys.sort_unstable();
            for key in keys {
                if let Some(item) = values.get(key) {
                    collect_features(item, &format!("{path}.{key}"), output);
                }
            }
        }
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x1000_0000_01b3;
const LEFT_SEED: u64 = 0x9e37_79b1_85eb_ca87;
const RIGHT_SEED: u64 = 0xc2b2_ae3d_27d4_eb4f;

fn feature_hashes(feature: &str) -> (u64, u64) {
    (
        fnv1a_64_with_seed(feature.as_bytes(), LEFT_SEED),
        fnv1a_64_with_seed(feature.as_bytes(), RIGHT_SEED),
    )
}

fn fnv1a_64_with_seed(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = FNV_OFFSET_BASIS ^ seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn bytes_to_hex(bytes: &[u8; FINGERPRINT_BYTES]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json::json;

    use crate::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };

    use super::{
        configuration_from_lines, FingerprintGenerator, FINGERPRINT_BITS, FINGERPRINT_BYTES,
    };

    #[test]
    fn generates_128_bit_fingerprint_and_vector() {
        let generator = FingerprintGenerator::new();
        let fingerprint = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 50 },
                { "id": "support-premium", "qty": 1 }
            ],
            "discount_pct": 15
        }));

        assert_eq!(fingerprint.hash_bytes.len(), FINGERPRINT_BYTES);
        assert_eq!(fingerprint.hash_hex.len(), FINGERPRINT_BYTES * 2);
        assert_eq!(fingerprint.vector.len(), FINGERPRINT_BITS);
    }

    #[test]
    fn same_configuration_is_deterministic_even_with_key_reordering() {
        let generator = FingerprintGenerator::new();

        let left = generator.generate_from_json(&json!({
            "discount_pct": 20,
            "products": [{"id": "plan-pro", "qty": 25}],
            "account_tier": "mid-market"
        }));
        let right = generator.generate_from_json(&json!({
            "account_tier": "mid-market",
            "products": [{"qty": 25, "id": "plan-pro"}],
            "discount_pct": 20
        }));

        assert_eq!(left, right);
        assert_eq!(generator.hamming_distance(&left, &right), 0);
    }

    #[test]
    fn quote_line_order_does_not_change_fingerprint() {
        let generator = FingerprintGenerator::new();
        let quote_a = quote_fixture(vec![
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
            },
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
            },
        ]);
        let quote_b = quote_fixture(vec![
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
            },
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
            },
        ]);

        let left = generator.generate_from_quote(&quote_a);
        let right = generator.generate_from_quote(&quote_b);
        assert_eq!(left, right);
    }

    #[test]
    fn similar_configurations_have_closer_signatures_than_distant_ones() {
        let generator = FingerprintGenerator::new();
        let base = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 100 },
                { "id": "support-premium", "qty": 1 }
            ],
            "term_months": 24,
            "discount_pct": 18
        }));
        let similar = generator.generate_from_json(&json!({
            "account_tier": "enterprise",
            "products": [
                { "id": "plan-enterprise", "qty": 105 },
                { "id": "support-premium", "qty": 1 }
            ],
            "term_months": 24,
            "discount_pct": 19
        }));
        let distant = generator.generate_from_json(&json!({
            "account_tier": "smb",
            "products": [
                { "id": "starter", "qty": 5 },
                { "id": "support-basic", "qty": 1 }
            ],
            "term_months": 12,
            "discount_pct": 0
        }));

        let similar_distance = generator.hamming_distance(&base, &similar);
        let distant_distance = generator.hamming_distance(&base, &distant);

        assert!(similar_distance < distant_distance);
        assert!(
            generator.similarity_score(&base, &similar)
                > generator.similarity_score(&base, &distant)
        );
    }

    #[test]
    fn canonical_line_configuration_is_stable() {
        let lines = vec![
            QuoteLine {
                product_id: ProductId("support-premium".to_owned()),
                quantity: 1,
                unit_price: Decimal::new(2_500, 2),
            },
            QuoteLine {
                product_id: ProductId("plan-enterprise".to_owned()),
                quantity: 100,
                unit_price: Decimal::new(15_000, 2),
            },
        ];
        let configuration = configuration_from_lines(&lines);
        assert_eq!(
            configuration,
            json!({
                "lines": [
                    {
                        "product_id": "plan-enterprise",
                        "quantity": 100,
                        "unit_price": "150"
                    },
                    {
                        "product_id": "support-premium",
                        "quantity": 1,
                        "unit_price": "25"
                    }
                ]
            })
        );
    }

    fn quote_fixture(lines: Vec<QuoteLine>) -> Quote {
        Quote {
            id: QuoteId("Q-2026-2001".to_owned()),
            status: QuoteStatus::Draft,
            lines,
            created_at: Utc::now(),
        }
    }
}
