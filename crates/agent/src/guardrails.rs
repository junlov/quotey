#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GuardrailPolicy {
    pub llm_can_set_prices: bool,
    pub llm_can_approve_discounts: bool,
}
