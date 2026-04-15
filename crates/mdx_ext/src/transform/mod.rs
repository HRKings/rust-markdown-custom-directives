//! AST transforms: normalization and directive resolution.

pub mod normalize;
pub mod resolve;

pub use normalize::normalize;
pub use resolve::resolve;
