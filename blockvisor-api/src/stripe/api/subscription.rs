use derive_more::{Deref, Display, From, FromStr};
use diesel_derive_newtype::DieselNewType;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SubscriptionId(String);

#[derive(Debug, serde::Deserialize)]
pub struct Subscription {
    /// Unique identifier for the object.
    pub id: SubscriptionId,
    /// Time at which the object was created.
    ///
    /// Measured in seconds since the Unix epoch.
    pub created: super::Timestamp,
    /// Three-letter [ISO currency code](https://www.iso.org/iso-4217-currency-codes.html), in
    /// lowercase.
    ///
    /// Must be a [supported currency](https://stripe.com/docs/currencies).
    pub currency: super::currency::Currency,
    /// End of the current period that the subscription has been invoiced for.
    ///
    /// At the end of this period, a new invoice will be created.
    pub current_period_end: super::Timestamp,
    /// Start of the current period that the subscription has been invoiced for.
    pub current_period_start: super::Timestamp,
    /// ID of the customer who owns the subscription.
    pub customer: super::IdOrObject<String, super::customer::Customer>,
    /// Number of days a customer has to pay invoices generated by this subscription.
    ///
    /// This value will be `null` for subscriptions where `collection_method=charge_automatically`.
    pub days_until_due: Option<u32>,
    /// ID of the default payment method for the subscription.
    ///
    /// It must belong to the customer associated with the subscription.
    /// This takes precedence over `default_source`.
    /// If neither are set, invoices will use the customer's invoice_settings.default_payment_method
    /// default_source.
    pub default_payment_method: Option<String>,
    /// The subscription's description, meant to be displayable to the customer.
    ///
    /// Use this field to optionally store an explanation of the subscription for rendering in
    /// Stripe surfaces and certain local payment methods UIs.
    pub description: Option<String>,
    /// If the subscription has ended, the date the subscription ended.
    pub ended_at: Option<super::Timestamp>,
    /// List of subscription items, each with an attached price.
    pub items: super::ListResponse<SubscriptionItem>,
    /// Has the value `true` if the object exists in live mode or the value `false` if the object
    /// exists in test mode.
    pub livemode: bool,
    /// Set of [key-value pairs](https://stripe.com/docs/api/metadata) that you can attach to an
    /// object.
    ///
    /// This can be useful for storing additional information about the object in a structured
    /// format.
    pub metadata: super::Metadata,
    /// Date when the subscription was first created.
    ///
    /// The date might differ from the `created` date due to backdating.
    pub start_date: super::Timestamp,
    /// Possible values are `incomplete`, `incomplete_expired`, `trialing`, `active`, `past_due`,
    /// `canceled`, or `unpaid`.
    ///
    /// For `collection_method=charge_automatically` a subscription moves into `incomplete` if the
    /// initial payment attempt fails. A subscription in this state can only have metadata and
    /// default_source updated. Once the first invoice is paid, the subscription moves into an
    /// `active` state. If the first invoice is not paid within 23 hours, the subscription
    /// transitions to `incomplete_expired`. This is a terminal state, the open invoice will be
    /// voided and no further invoices will be generated. A subscription that is currently in a
    /// trial period is `trialing` and moves to `active` when the trial period is over. If
    /// subscription `collection_method=charge_automatically`, it becomes `past_due` when payment is
    /// required but cannot be paid (due to failed payment or awaiting additional user actions).
    /// Once Stripe has exhausted all payment retry attempts, the subscription will become
    /// `canceled` or `unpaid` (depending on your subscriptions settings). If subscription
    /// `collection_method=send_invoice` it becomes `past_due` when its invoice is not paid by the
    /// due date, and `canceled` or `unpaid` if it is still not paid by an additional deadline after
    /// that. Note that when a subscription has a status of `unpaid`, no subsequent invoices will be
    /// attempted (invoices will be created, but then immediately automatically closed). After
    /// receiving updated payment information from a customer, you may choose to reopen and pay
    /// their closed invoices.
    pub status: SubscriptionStatus,
    /// If the subscription has a trial, the end of that trial.
    pub trial_end: Option<super::Timestamp>,
    /// If the subscription has a trial, the beginning of that trial.
    pub trial_start: Option<super::Timestamp>,
}

/// An enum representing the possible values of an `Subscription`'s `status` field.
#[derive(Debug, serde::Deserialize, derive_more::Display)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    Canceled,
    Incomplete,
    IncompleteExpired,
    PastDue,
    Paused,
    Trialing,
    Unpaid,
}

#[derive(
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Clone,
    Display,
    Hash,
    PartialEq,
    Eq,
    DieselNewType,
    Deref,
    From,
    FromStr,
)]
pub struct SubscriptionItemId(String);

/// The resource representing a Stripe "SubscriptionItem".
///
/// For more details see <https://stripe.com/docs/api/subscription_items/object>
#[derive(Debug, serde::Deserialize)]
pub struct SubscriptionItem {
    /// Unique identifier for the object.
    pub id: SubscriptionItemId,
    /// Time at which the object was created.
    ///
    /// Measured in seconds since the Unix epoch.
    pub created: Option<super::Timestamp>,
    // Always true for a deleted object
    #[serde(default)]
    pub deleted: bool,
    /// Set of [key-value pairs](https://stripe.com/docs/api/metadata) that you can attach to an
    /// object.
    ///
    /// This can be useful for storing additional information about the object in a structured
    /// format.
    pub metadata: Option<super::Metadata>,
    pub plan: Option<Plan>,
    pub price: Option<super::price::Price>,
    /// The [quantity](https://stripe.com/docs/subscriptions/quantities) of the plan to which the
    /// customer should be subscribed.
    #[serde(default = "default_quantity")]
    pub quantity: u64,
    /// The `subscription` this `subscription_item` belongs to.
    pub subscription: Option<String>,
}

fn default_quantity() -> u64 {
    1
}

#[derive(Debug, serde::Deserialize)]
pub struct PlanId(pub String);

/// The resource representing a Stripe "Plan".
///
/// For more details see <https://stripe.com/docs/api/plans/object>
#[derive(Debug, serde::Deserialize)]
pub struct Plan {
    /// Unique identifier for the object.
    pub id: PlanId,
    /// Whether the plan can be used for new purchases.
    pub active: Option<bool>,
    /// The unit amount in cents (or local equivalent) to be charged, represented as a whole integer
    /// if possible.
    ///
    /// Only set if `billing_scheme=per_unit`.
    pub amount: Option<i64>,
    /// The unit amount in cents (or local equivalent) to be charged, represented as a decimal
    /// string with at most 12 decimal places.
    ///
    /// Only set if `billing_scheme=per_unit`.
    pub amount_decimal: Option<String>,
    /// Time at which the object was created.
    ///
    /// Measured in seconds since the Unix epoch.
    pub created: Option<super::Timestamp>,
    /// Three-letter [ISO currency code](https://www.iso.org/iso-4217-currency-codes.html), in
    /// lowercase.
    ///
    /// Must be a [supported currency](https://stripe.com/docs/currencies).
    pub currency: Option<super::currency::Currency>,
    // Always true for a deleted object
    #[serde(default)]
    pub deleted: bool,
    /// The number of intervals (specified in the `interval` attribute) between subscription
    /// billings.
    ///
    /// For example, `interval=month` and `interval_count=3` bills every 3 months.
    pub interval_count: Option<u64>,
    /// Has the value `true` if the object exists in live mode or the value `false` if the object
    /// exists in test mode.
    pub livemode: Option<bool>,
    /// Set of [key-value pairs](https://stripe.com/docs/api/metadata) that you can attach to an
    /// object.
    ///
    /// This can be useful for storing additional information about the object in a structured
    /// format.
    pub metadata: Option<super::Metadata>,
    /// A brief description of the plan, hidden from customers.
    pub nickname: Option<String>,
    /// Default number of trial days when subscribing a customer to this plan using
    /// [`trial_from_plan=true`](https://stripe.com/docs/api#create_subscription-trial_from_plan).
    pub trial_period_days: Option<u32>,
}

/// The parameters for `Subscription::create`.
#[derive(Debug, serde::Serialize)]
pub struct CreateSubscription<'a> {
    customer: &'a str,
    #[serde(rename = "items[0][price]")]
    price_id: &'a super::price::PriceId,
    #[serde(rename = "items[0][quantity]")]
    quantity: u64,
}

impl<'a> CreateSubscription<'a> {
    pub const fn new(customer_id: &'a str, price_id: &'a super::price::PriceId) -> Self {
        Self {
            customer: customer_id,
            price_id,
            quantity: 1,
        }
    }
}

impl super::StripeEndpoint for CreateSubscription<'_> {
    type Result = Subscription;

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn path(&self) -> String {
        "subscriptions".to_string()
    }

    fn body(&self) -> Option<&Self> {
        Some(self)
    }
}

/// The parameters for `Subscription::list`.
#[derive(Debug, serde::Serialize, Default)]
pub struct ListSubscriptions<'a> {
    /// The ID of the customer whose subscriptions will be retrieved.
    #[serde(skip_serializing_if = "Option::is_none")]
    customer: Option<&'a str>,
}

impl<'a> ListSubscriptions<'a> {
    pub const fn new(customer_id: &'a str) -> Self {
        Self {
            customer: Some(customer_id),
        }
    }
}

impl super::StripeEndpoint for ListSubscriptions<'_> {
    type Result = super::ListResponse<Subscription>;

    fn method(&self) -> reqwest::Method {
        reqwest::Method::GET
    }

    fn path(&self) -> String {
        "subscriptions".to_string()
    }

    fn query(&self) -> Option<&Self> {
        Some(self)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CreateSubscriptionItem<'a> {
    subscription: &'a SubscriptionId,
    price: &'a super::price::PriceId,
    quantity: u64,
}

impl<'a> CreateSubscriptionItem<'a> {
    pub const fn new(
        subscription_id: &'a SubscriptionId,
        price: &'a super::price::PriceId,
    ) -> Self {
        Self {
            subscription: subscription_id,
            price,
            quantity: 1,
        }
    }
}

impl super::StripeEndpoint for CreateSubscriptionItem<'_> {
    type Result = SubscriptionItem;

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn path(&self) -> String {
        "subscription_items".to_string()
    }

    fn body(&self) -> Option<&Self> {
        Some(self)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GetSubscriptionItem<'a> {
    item_id: &'a SubscriptionItemId,
}

impl<'a> GetSubscriptionItem<'a> {
    pub const fn new(item_id: &'a SubscriptionItemId) -> Self {
        Self { item_id }
    }
}

impl super::StripeEndpoint for GetSubscriptionItem<'_> {
    type Result = SubscriptionItem;

    fn method(&self) -> hyper::Method {
        hyper::Method::GET
    }

    fn path(&self) -> String {
        format!("subscription_items/{}", self.item_id)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ListSubscriptionItems<'a> {
    subscription: &'a SubscriptionId,
}

impl<'a> ListSubscriptionItems<'a> {
    pub const fn new(id: &'a SubscriptionId) -> Self {
        Self { subscription: id }
    }
}

impl super::StripeEndpoint for ListSubscriptionItems<'_> {
    type Result = super::ListResponse<SubscriptionItem>;

    fn method(&self) -> hyper::Method {
        reqwest::Method::GET
    }

    fn path(&self) -> String {
        "subscription_items".to_string()
    }

    fn query(&self) -> Option<&Self> {
        Some(self)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct UpdateSubscriptionItem<'a> {
    id: &'a SubscriptionItemId,
    quantity: Option<u64>,
}

impl<'a> UpdateSubscriptionItem<'a> {
    pub const fn new(id: &'a SubscriptionItemId, quantity: u64) -> Self {
        Self {
            id,
            quantity: Some(quantity),
        }
    }
}

impl super::StripeEndpoint for UpdateSubscriptionItem<'_> {
    type Result = SubscriptionItem;

    fn method(&self) -> reqwest::Method {
        reqwest::Method::POST
    }

    fn path(&self) -> String {
        format!("subscription_items/{}", self.id)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DeleteSubscriptionItem<'a> {
    item_id: &'a str,
}

impl<'a> DeleteSubscriptionItem<'a> {
    pub const fn new(item_id: &'a str) -> Self {
        Self { item_id }
    }
}

impl super::StripeEndpoint for DeleteSubscriptionItem<'_> {
    type Result = super::DeleteResponse;

    fn method(&self) -> reqwest::Method {
        reqwest::Method::DELETE
    }

    fn path(&self) -> String {
        format!("subscription_items/{}", self.item_id)
    }
}
