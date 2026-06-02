//! Built-in minimizer factories.

pub mod in_range;
pub mod known_value;
pub mod one_hot;
pub mod spy;

pub use in_range::create_in_range_factor;
pub use known_value::create_known_value_factor;
pub use one_hot::create_one_hot_factor;
pub use spy::create_spy_factor;
